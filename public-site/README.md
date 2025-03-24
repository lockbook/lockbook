Hey welcome to lockbook's public site. 

technologies used:
- [zola](https://www.getzola.org/): a static website generator to power the blog 
- [trunk](https://trunkrs.dev/): packages workspace into a wasm build 
- [tailwind](https://trunkrs.dev/): css styling

## Build 

to build the website, use trunk to package workspace into a wasm binary included in a html page. then this html page is used as a zola template to be leveraged in zola static site generation pipeline. 
All of this can be done though `./build.sh` which will also start a development server on [http://localhost:5500/](http://localhost:5500/)

ensure that you have `trunk`, `zola` and added `wasm` as rust compilation target. this can be done through `./setup_macos.sh`

## Deploy

to deploy to gcp, run `./build.sh --deploy`

you might have to setup gcloud auth `gcloud auth login`