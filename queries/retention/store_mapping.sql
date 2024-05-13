INSERT OR REPLACE INTO mapping_retention (
    topic,
    major_version,
    qos,
    expiry_sec
) VALUES (?, ?, ?, ?)
