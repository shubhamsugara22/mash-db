CREATE TABLE test_median (id INT, value INT);
INSERT INTO test_median
VALUES (1, 10);
INSERT INTO test_median
VALUES (2, 30);
INSERT INTO test_median
VALUES (3, 20);
INSERT INTO test_median
VALUES (4, 50);
INSERT INTO test_median
VALUES (5, 40);
SELECT MEDIAN(value)
FROM test_median;