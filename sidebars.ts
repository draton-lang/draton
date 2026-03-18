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
        {
          type: 'category',
          label: 'Syntax Reference',
          items: [
            'language/reference/index',
            'language/reference/literals-and-values',
            'language/reference/bindings-and-assignment',
            'language/reference/functions-calls-and-lambdas',
            'language/reference/expressions-and-operators',
            'language/reference/control-flow-and-patterns',
            'language/reference/top-level-items-and-modules',
            'language/reference/types-and-contracts',
            'language/reference/classes-interfaces-enums-and-errors',
            'language/reference/concurrency-and-channels',
            'language/reference/low-level-and-compile-time'
          ]
        },
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
