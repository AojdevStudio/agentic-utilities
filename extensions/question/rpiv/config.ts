import type { GuidanceFields } from "@juicesharp/rpiv-config";
import { configPath, loadJsonConfig, validateGuidanceFields } from "@juicesharp/rpiv-config";

const CONFIG_PATH = configPath("rpiv-ask-user-question");

interface AskUserQuestionConfig {
  guidance?: GuidanceFields;
}

export function loadConfig(): AskUserQuestionConfig {
  return loadJsonConfig<AskUserQuestionConfig>(CONFIG_PATH);
}

export { validateGuidanceFields };
