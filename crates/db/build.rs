// `sqlx::migrate!` embeds the migrations at compile time; rebuild this crate
// when one is added or changed, or tests would run against a stale set.
fn main() {
    println!("cargo:rerun-if-changed=../../migrations");
}
