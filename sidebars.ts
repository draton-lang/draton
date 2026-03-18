import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  mainSidebar: [
    'intro',
    {
      type: 'category',
      label: 'Getting Started',
      items: [
        'getting-started/overview',
        'install',
        'quickstart',
        'early-preview'
      ]
    },
    {
      type: 'category',
      label: 'Language',
      items: [
        'language-manifesto',
        'language-architecture',
        'language/syntax-overview',
        'language/contracts-and-types',
        'language/modules-and-structure',
        'language/control-flow-and-builtins',
        'canonical-syntax-rules',
        'syntax-migration',
        'language-class-diagram',
        'language-analyst-artifact'
      ]
    },
    {
      type: 'category',
      label: 'Tooling',
      items: [
        'tooling/cli-overview',
        'tools/formatter',
        'tools/linter',
        'tools/task',
        'tools/lsp'
      ]
    },
    {
      type: 'category',
      label: 'Compiler and Runtime',
      items: [
        'compiler-architecture',
        'runtime/runtime-and-gc',
        'gc-scorecard',
        'runtime/benchmarks',
        'selfhost-canonical-migration-status'
      ]
    },
    {
      type: 'category',
      label: 'Contributors and Policy',
      items: [
        'contributor-language-rules',
        'contributor/docs-site-deployment',
        'release-workflow',
        'roadmap-1year'
      ]
    }
  ]
};

export default sidebars;
