# Sirene AppImage runtime source

This archive accompanies the Sirene AppImage for the same architecture. The AppImage starts with
a small statically linked launcher, followed by the Sirene application in a zstd SquashFS
filesystem. The license texts under `licenses/` are also installed inside that filesystem.
The launcher is built from the pinned AppImage type 2 runtime, libfuse, squashfuse, and zstd.
The Alpine toolchain supplies musl and the C build tools. `build-packages.txt` records its
installed package versions.

`sources/` contains the full source archives used by the build. The libfuse 3.15.0 archive
and the libfuse patch under `patches/libfuse/` in `type2-runtime.tar.gz` are the libfuse source
for this runtime. The `relink/` directory holds the launcher object, linker script, and
static application libraries from the build.

To rebuild the runtime on a machine of the same architecture, extract this archive, then
run the following command from the extracted directory. Docker pulls the recorded Alpine base
and the recipe installs the build tools in the container.

```sh
docker run --rm --volume "$PWD:/work" --env "SIRENE_RUNTIME_OWNER=$(id -u):$(id -g)" \
    "$(cat builder-image)" sh /work/build-runtime.sh
```

To relink the original launcher objects with a modified libfuse, replace
`sources/fuse-3.15.0.tar.xz` with the modified source archive in the same layout, then run
the same command with `relink` after `/work/build-runtime.sh`. The recipe reapplies the
bundled mount patch. Keep that patch applicable to your modified source.

To run the original application through the resulting `runtime`, take the SquashFS from the
original AppImage and append it to the newly built runtime.

```sh
offset=$(./Sirene.AppImage --appimage-offset)
cp runtime Sirene-relinked.AppImage
tail -c "+$((offset + 1))" Sirene.AppImage >> Sirene-relinked.AppImage
chmod +x Sirene-relinked.AppImage
./Sirene-relinked.AppImage --appimage-extract-and-run --version
```
