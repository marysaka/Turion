fn main() {
    println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libBambuSource.so");
}
