(use-modules (gnu packages commencement)
             (gnu packages llvm)
             (gnu packages pkg-config)
             (gnu packages freedesktop)
             (gnu packages xdisorg)
             (gnu packages vulkan)
             (gnu packages rust)
             (gnu packages linux))

(packages->manifest
 (list rust
       (list rust "cargo")
       gcc-toolchain
       clang-toolchain
       pkg-config
       wayland
       wayland-protocols
       libxkbcommon
       vulkan-loader
       eudev))
