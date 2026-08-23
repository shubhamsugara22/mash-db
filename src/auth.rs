use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

const SALT_BYTES: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub username: String,
    pub role: String,
    salt: String,
    password_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionGrant {
    pub username: String,
    pub privilege: String,
    pub table_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthCatalog {
    pub accounts: HashMap<String, Account>,
    pub grants: Vec<PermissionGrant>,
}

impl AuthCatalog {
    pub fn load(path: impl AsRef<Path>) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        std::fs::write(path, json).map_err(|error| error.to_string())
    }

    pub fn create_account(
        &mut self,
        username: &str,
        password: &str,
        role: &str,
    ) -> Result<(), String> {
        let username = normalize_identifier(username)?;
        let role = normalize_identifier(role)?;
        if password.is_empty() {
            return Err("Password cannot be empty".to_string());
        }
        if self.accounts.contains_key(&username) {
            return Err(format!("User '{}' already exists", username));
        }

        let mut salt = [0_u8; SALT_BYTES];
        OsRng.fill_bytes(&mut salt);
        let salt_hex = to_hex(&salt);
        let password_hash = hash_password(&salt_hex, password);
        self.accounts.insert(
            username.clone(),
            Account {
                username,
                role,
                salt: salt_hex,
                password_hash,
            },
        );
        Ok(())
    }

    pub fn verify_password(&self, username: &str, password: &str) -> bool {
        let username = username.to_lowercase();
        self.accounts
            .get(&username)
            .map(|account| hash_password(&account.salt, password) == account.password_hash)
            .unwrap_or(false)
    }

    pub fn grant(
        &mut self,
        username: &str,
        privilege: &str,
        table_name: &str,
    ) -> Result<(), String> {
        let username = normalize_identifier(username)?;
        let privilege = normalize_identifier(privilege)?;
        let table_name = normalize_identifier(table_name)?;
        if !self.accounts.contains_key(&username) {
            return Err(format!("User '{}' does not exist", username));
        }
        if !self.grants.iter().any(|grant| {
            grant.username == username
                && grant.privilege == privilege
                && grant.table_name == table_name
        }) {
            self.grants.push(PermissionGrant {
                username,
                privilege,
                table_name,
            });
        }
        Ok(())
    }

    pub fn revoke(&mut self, username: &str, privilege: &str, table_name: &str) -> bool {
        let username = username.to_lowercase();
        let privilege = privilege.to_lowercase();
        let table_name = table_name.to_lowercase();
        let before = self.grants.len();
        self.grants.retain(|grant| {
            !(grant.username == username
                && grant.privilege == privilege
                && grant.table_name == table_name)
        });
        self.grants.len() != before
    }

    pub fn has_grant(&self, username: &str, privilege: &str, table_name: &str) -> bool {
        let username = username.to_lowercase();
        let privilege = privilege.to_lowercase();
        let table_name = table_name.to_lowercase();
        self.accounts
            .get(&username)
            .map(|account| account.role == "admin")
            .unwrap_or(false)
            || self.grants.iter().any(|grant| {
                grant.username == username
                    && grant.privilege == privilege
                    && (grant.table_name == table_name || grant.table_name == "*")
            })
    }
}

fn normalize_identifier(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_lowercase();
    if normalized.is_empty()
        || !normalized
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(format!("Invalid identifier '{}'", value));
    }
    Ok(normalized)
}

fn hash_password(salt: &str, password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b":");
    hasher.update(password.as_bytes());
    to_hex(&hasher.finalize())
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{:02x}", byte)).collect()
}

#[cfg(test)]
mod tests {
    use super::AuthCatalog;

    #[test]
    fn account_password_is_hashed_and_verifiable() {
        let mut catalog = AuthCatalog::default();
        catalog.create_account("Alice", "secret", "reader").unwrap();

        assert!(catalog.verify_password("alice", "secret"));
        assert!(!catalog.verify_password("alice", "wrong"));
        let account = catalog.accounts.get("alice").unwrap();
        assert!(!account.password_hash.contains("secret"));
    }

    #[test]
    fn grants_can_be_added_checked_and_revoked() {
        let mut catalog = AuthCatalog::default();
        catalog.create_account("alice", "secret", "reader").unwrap();

        catalog.grant("alice", "select", "products").unwrap();
        assert!(catalog.has_grant("alice", "select", "products"));
        assert!(!catalog.has_grant("alice", "insert", "products"));
        assert!(catalog.revoke("alice", "select", "products"));
        assert!(!catalog.has_grant("alice", "select", "products"));
    }

    #[test]
    fn admin_has_all_grants() {
        let mut catalog = AuthCatalog::default();
        catalog.create_account("admin", "secret", "admin").unwrap();

        assert!(catalog.has_grant("admin", "drop", "anything"));
    }
}
