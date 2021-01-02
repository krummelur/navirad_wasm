# navirad_wasm
Rust port of the rendering code for navirad, builds to WASM.  
Just a first experiment with WASM; I was curious if a port to wasm would be faster than the js code. It was faster by a factor of ~2.  
Introducing parallelism with web-workers would likely have been faster.

wasm-pack must be installed to build.

### cargo build
Compiles dependencies and project.

### wasm-pack build
packages the project as wasm into /pkg.
