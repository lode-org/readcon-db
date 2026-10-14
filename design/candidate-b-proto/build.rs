fn main() {
    println!("cargo:rustc-link-search=native=/tmp/prefix/lib");
    println!("cargo:rustc-link-lib=static=zfp");
    println!("cargo:rustc-link-lib=static=blosc2");
    println!("cargo:rustc-link-lib=pthread");
    println!("cargo:rustc-link-lib=m");
    println!("cargo:rerun-if-changed=build.rs");
}
