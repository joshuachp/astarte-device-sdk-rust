INSERT OR REPLACE INTO mapping_retention (
    topic,
    major_version,
    reliability,
    expiry_sec
) VALUES (?, ?, ?, ?)
