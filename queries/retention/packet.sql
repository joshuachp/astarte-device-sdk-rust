SELECT
    t_millis,
    counter AS "counter: u32",
    path,
    payload
FROM mapping_packet
WHERE t_millis = ? AND counter = ?;
