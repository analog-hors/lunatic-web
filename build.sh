wasm-pack build "${1:-"--dev"}" --target no-modules --out-name lunatic
echo ";wasm_bindgen();" >> ./pkg/lunatic.js
cp -r ./static/* ./pkg
