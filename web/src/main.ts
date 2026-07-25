import { createApp } from "vue";
import { createThemeProvider } from "@quantumaudio/ableton-extension-sdk";
import "@quantumaudio/ableton-extension-sdk/theme.css";
import "@quantumaudio/ableton-extension-sdk/styles.css";
import "./styles/layout.css";
import App from "./App.vue";

const boot = document.documentElement.getAttribute("data-theme-boot");
const initial =
  boot === "light" || boot === "system"
    ? "light"
    : "dark";

createThemeProvider(document.documentElement, { defaultTheme: initial });
document.documentElement.setAttribute("data-qa-theme", initial);

createApp(App).mount("#app");
