import wasmModule from "./build/profile_metrics.wasm";
import { createHandler } from "./handler.js";

export default createHandler(wasmModule);
