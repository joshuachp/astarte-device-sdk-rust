SELECT
    path,
    major_version as "major_version: i32",
    reliability as "qos: u8",
    expiry_sec
FROM mapping_retention
WHERE path = ?
