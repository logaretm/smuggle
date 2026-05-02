import DefaultTheme from "vitepress/theme";
import ProcessDiagram from "../components/ProcessDiagram.vue";

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component("ProcessDiagram", ProcessDiagram);
  },
};
