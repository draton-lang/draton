import clsx from 'clsx';
import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';

import styles from './index.module.css';

const sections = [
  {
    title: 'Learn the language',
    text: 'Syntax, contracts, control flow, and the ownership model — everything you need to write real Draton, in one place.',
    to: '/docs/language/syntax-overview'
  },
  {
    title: 'Get it running',
    text: 'Install the CLI, run your first program, and understand what "early preview" actually means for your workflow.',
    to: '/docs/install'
  },
  {
    title: 'See how it works',
    text: 'The compiler pipeline, inferred ownership, bare-metal runtime layers, and how self-hosting fits in.',
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
      title="Draton — a language that gets out of your way"
      description="Docs for the Draton language: syntax, inferred ownership, compiler, runtime, and tooling."
    >
      <main className={styles.page}>
        <section className={styles.hero}>
          <div className={styles.heroPanel}>
            <p className={styles.kicker}>Early Tooling Preview</p>
            <Heading as="h1" className={styles.title}>
              A language that gets out of your way.
            </Heading>
            <p className={styles.subtitle}>
              Draton is a statically-typed compiled language with inferred ownership — no GC pauses,
              no lifetime annotations, no boilerplate. These docs cover the language, the toolchain,
              and everything in between.
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
            <Heading as="h2">What's here</Heading>
            <p>
              Everything from first principles to internals. Whether you're writing your first
              Draton program or digging into how the ownership checker works, it's in here.
            </p>
            <ul className={styles.bullets}>
              <li>Language: syntax, contracts, control flow, modules, classes, and builtins.</li>
              <li>Tooling: build, run, format, lint, tasks, and the LSP.</li>
              <li>Compiler &amp; runtime: pipeline, inferred ownership, bare-metal layers, and self-hosting.</li>
              <li>Contributing: anti-drift policy, migration boundaries, and release workflow.</li>
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
