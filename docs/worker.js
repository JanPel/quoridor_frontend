import init from "/quoridor_frontend/assets/dioxus/quoridor_frontend.js";

init("/quoridor_frontend/assets/dioxus/quoridor_frontend_bg.wasm").then(wasm => {
  wasm.start_webworker();
});
