pub mod sensor;

#[cfg(test)]
mod smoke_test {
    use rusqlite::Connection;

    #[test]
    fn fts5_with_porter_tokenizer_works() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE VIRTUAL TABLE t USING fts5(name, tokenize='porter unicode61');
             INSERT INTO t(name) VALUES ('ESP32-S3');",
        )
        .expect("FTS5 must be available in bundled rusqlite");
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM t WHERE t MATCH 'ESP32*'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }
}
