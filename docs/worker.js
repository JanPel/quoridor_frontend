import init from "/assets/dioxus/quoridor_frontend.js";

init("/assets/dioxus/quoridor_frontend_bg.wasm").then(wasm => {
  wasm.start_webworker();
});
