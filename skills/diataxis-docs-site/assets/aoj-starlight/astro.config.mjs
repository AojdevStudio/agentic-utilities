import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";

export default defineConfig({
  site: __SITE_URL_JSON__,
  base: __BASE_PATH_JSON__,
  integrations: [
    starlight({
      title: __PROJECT_NAME_JSON__,
      description: __DESCRIPTION_JSON__,
      favicon: "/favicon.svg",
      customCss: ["./src/styles/aoj-docs.css"],
      lastUpdated: true,
      sidebar: [
        {
          label: "Tutorials",
          items: [{ autogenerate: { directory: "tutorials" } }],
        },
        {
          label: "How-to guides",
          items: [{ autogenerate: { directory: "how-to" } }],
        },
        {
          label: "Reference",
          items: [{ autogenerate: { directory: "reference" } }],
        },
        {
          label: "Explanation",
          items: [{ autogenerate: { directory: "explanation" } }],
        },
        {
          label: "Project",
          items: [{ label: "Source repository", link: __REPOSITORY_URL_JSON__ }],
        },
      ],
    }),
  ],
});
