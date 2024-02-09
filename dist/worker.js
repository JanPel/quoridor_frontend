import init from "/./assets/dioxus/name.js";

init("/./assets/dioxus/name_bg.wasm").then(wasm => {
  wasm.start_webworker();
});
