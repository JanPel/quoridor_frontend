import init from "/./assets/dioxus/Quoridor.js";

init("/./assets/dioxus/Quoridor_bg.wasm").then(wasm => {
  wasm.start_webworker();
});
