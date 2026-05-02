import DefaultTheme from "vitepress/theme";
import "./custom.css";
import ProcessDiagram from "../components/ProcessDiagram.vue";
import HeroFeatures from "../components/HeroFeatures.vue";
import TerminalDemo from "../components/TerminalDemo.vue";
import BeforeAfter from "../components/BeforeAfter.vue";
import Principles from "../components/Principles.vue";
import InstallTabs from "../components/InstallTabs.vue";
import Steps from "../components/Steps.vue";
import Callout from "../components/Callout.vue";
import CommandsOverview from "../components/CommandsOverview.vue";

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component("ProcessDiagram", ProcessDiagram);
    app.component("HeroFeatures", HeroFeatures);
    app.component("TerminalDemo", TerminalDemo);
    app.component("BeforeAfter", BeforeAfter);
    app.component("Principles", Principles);
    app.component("InstallTabs", InstallTabs);
    app.component("Steps", Steps);
    app.component("Callout", Callout);
    app.component("CommandsOverview", CommandsOverview);
  },
};
