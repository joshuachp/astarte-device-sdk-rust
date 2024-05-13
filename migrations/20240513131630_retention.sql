-- Interface stored retention
CREATE TABLE IF NOT EXISTS mapping_retention (
    -- Interface topic where the data was published on.
    topic TEXT NOT NULL PRIMARY KEY,
    -- Version of the interface the data was published on.
    major_version INTEGER NOT NULL,
    -- Quality of service
    qos INTEGER NOT NULL,
    -- Seconds after the entry will expire
    expiry_sec INTEGER
);


-- Interface stored retention
CREATE TABLE IF NOT EXISTS mapping_packet (
    -- Timestamp as u128 milliseconds since the Unix epoch, used for packet order
    t_millis BLOB NOT NULL,
    --- Counter for same milliseconds packets
    counter INTEGER NOT NULL,
    --- Topic of the packet
    topic TEXT NOT NULL,
    -- Payload for the packet
    payload BLOB NOT NULL,
    -- Primary key for packet uniqueness and ordering the table ordering
    PRIMARY KEY (t_millis, counter),
    -- References to the retention information
    FOREIGN KEY (topic) REFERENCES mapping_retention (
        topic
    ) ON UPDATE CASCADE ON DELETE CASCADE
);
