import clsx from 'clsx';
import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';

import styles from './index.module.css';

const sections = [
  {
    title: 'Start with the language',
    text: 'Read the canonical overview, learn the syntax surface, and understand the contract model without picking through unrelated contributor notes.',
    to: '/docs/language/syntax-overview'
  },
  {
    title: 'Install and run Draton',
    text: 'Use the install guide, quickstart, and early preview notes to get from zero to a running project with the official CLI.',
    to: '/docs/install'
  },
  {
    title: 'Understand the toolchain',
    text: 'Follow the compiler, runtime, and self-host docs to see how the Rust frontend, runtime, and mirror fit together.',
    to: '/docs/compiler-architecture'
  }
];

const quickLinks = [
  {label: 'Docs overview', to: '/docs'},
  {label: 'Install', to: '/docs/install'},
  {label: 'Quickstart', to: '/docs/quickstart'},
  {label: 'Language architecture', to: '/docs/language-architecture'},
  {label: 'CLI overview', to: '/docs/tooling/cli-overview'},
  {label: 'GitHub Releases', href: 'https://github.com/draton-lang/draton/releases'}
];

export default function Home(): JSX.Element {
  return (
    <Layout
      title="Draton language and toolchain documentation"
      description="Authoritative docs for the Draton language, compiler, runtime, tooling, and self-host mirror."
    >
      <main className={styles.page}>
        <section className={styles.hero}>
          <div className={styles.heroPanel}>
            <p className={styles.kicker}>Documentation</p>
            <Heading as="h1" className={styles.title}>
              Draton, documented as a language and toolchain.
            </Heading>
            <p className={styles.subtitle}>
              This site is the operational manual for Draton: canonical syntax, language structure,
              compiler architecture, runtime behavior, tooling, install paths, and contributor
              rules.
            </p>
            <div className={styles.actions}>
              <Link className="button button--primary button--lg" to="/docs">
                Open the docs
              </Link>
              <Link className="button button--secondary button--lg" to="/docs/quickstart">
                Quickstart
              </Link>
            </div>
          </div>
        </section>

        <section className={styles.gridSection}>
          <div className={styles.grid}>
            {sections.map((section) => (
              <Link key={section.title} to={section.to} className={clsx(styles.card, styles.primaryCard)}>
                <Heading as="h2">{section.title}</Heading>
                <p>{section.text}</p>
              </Link>
            ))}
          </div>
        </section>

        <section className={styles.detailSection}>
          <div className={styles.detailPanel}>
            <Heading as="h2">What this site covers</Heading>
            <p>
              The docs are organized around Draton as it actually exists in this repository: a
              readability-first language with one canonical syntax lane, a Rust toolchain as the
              source of truth, an operational self-host mirror, and a CLI-centered early tooling
              ecosystem.
            </p>
            <ul className={styles.bullets}>
              <li>Language guide: syntax, contracts, control flow, modules, classes, layers, and builtins.</li>
              <li>Tooling guide: build, run, format, lint, tasks, and the language server.</li>
              <li>Compiler and runtime guide: pipeline, runtime model, GC scorecards, and self-host boundary.</li>
              <li>Contributor rules: anti-drift policy, migration boundaries, release workflow, and roadmap.</li>
            </ul>
          </div>
          <div className={styles.linkPanel}>
            <Heading as="h2">Quick links</Heading>
            <div className={styles.linkList}>
              {quickLinks.map((link) =>
                link.href ? (
                  <a key={link.label} href={link.href} className={styles.quickLink}>
                    {link.label}
                  </a>
                ) : (
                  <Link key={link.label} to={link.to ?? '/docs'} className={styles.quickLink}>
                    {link.label}
                  </Link>
                ),
              )}
            </div>
          </div>
        </section>
      </main>
    </Layout>
  );
}
