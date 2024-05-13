SELECT
    topic,
    major_version as "major_version: i32",
    qos as "qos: u8",
    expiry_sec
FROM mapping_retention
WHERE topic = ?
