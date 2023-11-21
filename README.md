# omecat

Don't use this code. Some utils for parsing/combining OME-XML metadata...

```sh
cargo run concat ~/Downloads/VAN0038-LK-4-01-preAF-MxIF-3d-registered.ome.tiff \
  --size-z 99 \
  --physical-size-z 1.0 \
  --physical-size-z-unit µm \
  --filename-template VAN0038-LK-4-{z}-01-preAF-MXIF-3d-registered.ome.tiff
```
