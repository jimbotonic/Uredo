//! Creates the database the compile-time-checked queries are checked against, so the fixture needs
//! no external tool and no running server: the driver itself makes the file, and `DATABASE_URL` is
//! set for this crate's own compilation.

use std::env;
use std::path::Path;

fn main() {
    let out = env::var("OUT_DIR").expect("OUT_DIR");
    let db = Path::new(&out).join("compat.db");
    let url = format!("sqlite:{}", db.display());
    let _ = std::fs::remove_file(&db);

    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
    rt.block_on(async {
        use sqlx::sqlite::SqliteConnectOptions;
        use sqlx::{ConnectOptions, Executor};
        let mut conn = SqliteConnectOptions::new()
            .filename(&db)
            .create_if_missing(true)
            .connect()
            .await
            .expect("create the database");
        conn.execute("create table widget (id integer primary key, name text not null)")
            .await
            .expect("create the schema");
    });

    println!("cargo::rustc-env=DATABASE_URL={}", url);
    println!("cargo::rerun-if-changed=build.rs");
}
