# Third-party software

`twin-modeling-mcp` serves three.js, OrbitControls and TransformControls to its local 3D page. The page also follows
the look of the Anatomy skill (design values only, no code). All third-party parts are under the MIT licence. Each library
folder in `crates/twin-page/ui/vendor/` holds its own licence file. The files are the original files of the release, with no
edit. The file `ui/vendor/.gitattributes` turns off line-ending conversion, so the SHA-256 values hold on every checkout.
All paths below are relative to `crates/twin-page/`.

three.js r187 has no `.min.js` build in its official npm package and GitHub release (the `build/` folder holds
`three.module.js` and `three.core.js` only). The files here are the official non-minified build. The page maps `three` to
`three.module.js` (it imports `./three.core.js`) and `three/addons/` to `ui/vendor/three/` with an import map.

| Name | Version | License | Official source | Files (path, size, SHA-256) |
|---|---|---|---|---|
| three.js | r187 (npm 0.187.0) | MIT | https://github.com/mrdoob/three.js (npm: https://registry.npmjs.org/three/-/three-0.187.0.tgz) | `ui/vendor/three/three.module.js` 658900 B sha256 104f56145899722094556490d787aae0617cc2fc3382f31df02d724ea1aee341 |
| three.js | r187 | MIT | same | `ui/vendor/three/three.core.js` 1474799 B sha256 613e370f76638fdef83926642bf256dc9615684922de1ce1f63a8583e5d78578 |
| three.js | r187 | MIT | same | `ui/vendor/three/LICENSE` 1081 B sha256 8b378ebe60e2fe500158cb0ac71cb5e8b7d92953c2abcc63a0eb90499653b5bc |
| three.js addon OrbitControls | r187 | MIT | same (`examples/jsm/controls/OrbitControls.js`) | `ui/vendor/three/controls/OrbitControls.js` 40918 B sha256 e7fca2fe9e7670a828af9b28084669a5909445b7f36f1f26cee9bf8f2961c32c |
| three.js addon TransformControls | r187 | MIT | same | `ui/vendor/three/controls/TransformControls.js` 51706 B sha256 151befe25bb0d68626f9a6b033625b7b0cf6848e39ba71623b7bed8e3df565d6 |
| three.js addon GLTFLoader | r187 | MIT | same | `ui/vendor/three/loaders/GLTFLoader.js` 119861 B sha256 532ac726e120039acbcd6cf54328c57acbf7b438176afaa6951257126d11bff1 |
| three.js addon STLLoader | r187 | MIT | same | `ui/vendor/three/loaders/STLLoader.js` 10788 B sha256 a07441b8cd812faf8049741fe5e95373b19547e4038694753dfec26fe230a7eb |
| three.js addon BufferGeometryUtils (import of GLTFLoader) | r187 | MIT | same | `ui/vendor/three/utils/BufferGeometryUtils.js` 39153 B sha256 7bda2a8ba836b2bcf67cb938d4b9e1bb5d1b1bc616817103ed858c0b58296ce7 |
| three.js addon SkeletonUtils (import of GLTFLoader) | r187 | MIT | same | `ui/vendor/three/utils/SkeletonUtils.js` 11535 B sha256 b1632a703206c3d830de9fcbe515696770d04b71a15ee6b50afa6d2c3298c86f |
| meshoptimizer decoder | 1.3.0 | MIT | https://github.com/zeux/meshoptimizer (npm: https://registry.npmjs.org/meshoptimizer/-/meshoptimizer-1.3.0.tgz) | `ui/vendor/meshopt/meshopt_decoder.module.js` 29019 B sha256 10b313edaa0e322b0c1b46443f6c8b186b42139fe1eca189d50c09511d0ebedc (npm file `meshopt_decoder.mjs`, renamed, content unchanged) |
| meshoptimizer decoder | 1.3.0 | MIT | same | `ui/vendor/meshopt/LICENSE` 1079 B sha256 f03037ca7bad1e3eb7f4a63fa6084a8baabd5ba30d3c239a9a7f35705d873e26 (npm file `LICENSE.md`, renamed) |

The exe serves only a fixed list of files at `/vendor/<name>`: `three/three.module.js`, `three/three.core.js`,
`three/controls/OrbitControls.js`, `three/controls/TransformControls.js` and the project's own `twin-scene.js` and
`twin-core.js` (not third-party). The loaders, their two utility files and the meshoptimizer decoder are kept for a later
model phase; the exe does not serve them.

Check the SHA-256 values:

```bash
cd crates/twin-page/ui/vendor && find . -type f ! -name .gitattributes -exec sha256sum {} \; | sort -k2
```

## Pictures

The pictures in `examples/` and `docs/assets/` are screenshots of this program's own 3D page, made for this
repository. No third-party image is used.

## Design values (no code): Anatomy

The look of the 3D page (`ui/twin-model.html`, `ui/twin-scene.js`) follows the open-source skill "Anatomy". No Anatomy code
or file is in this repository; the look is rebuilt with three.js. These values were copied:

| Name | Version | License | Official source | What was copied |
|---|---|---|---|---|
| Anatomy | branch `main`, commit `1482e8469b0a` | MIT | https://github.com/wheresryan22/anatomy (`kit/iso.css`, `references/craft.md`) | The paper, shade, line-tone and accent color values (the `--a-*`, `--coil`, `--sensor`, `--alarm`, `--exhaust` tokens in the page), the four-tone shading steps and the camera angles (azimuth 45°, elevation 30°) in `ui/twin-scene.js` |

```
MIT License

Copyright (c) 2026 Ryan

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
