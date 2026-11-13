fn main() {
    println!("cargo:rerun-if-env-changed=READCON_DB_CORE_0_23");
    if std::env::var("READCON_DB_CORE_0_23").ok().as_deref() == Some("1") {
        println!("cargo:rustc-cfg=readcon_core_0_23");
    }
}
