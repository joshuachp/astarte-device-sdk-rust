SELECT
    mapping_packet.t_millis,
    mapping_packet.counter AS "counter: u32",
    mapping_packet.topic,
    mapping_packet.payload,
    mapping_retention.qos AS "qos: u8"
FROM mapping_packet
INNER JOIN mapping_retention USING (topic)
ORDER BY t_millis ASC, counter ASC
