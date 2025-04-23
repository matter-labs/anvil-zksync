// vocs.config.ts
import { resolve } from 'node:path';
import { defineConfig } from 'vocs';

export default defineConfig({
  // TODO: determine why this is crashing intermittently
  // aiCta: {
  //   query({ location }) {
  //     return `Please research and analyze this page: ${location} so I can ask you questions about it. Once you have read it, prompt me with any questions I have. Do NOT post content from the page in your response. Any of my follow up questions MUST reference the site I gave you.`;
  //   },
  // },
  description:
    'Light weight, fast, and easy to use Elastic Network development environment. Fork any ZK chain and run it locally with anvil-zksync.',
  editLink: {
    pattern: 'https://github.com/matter-labs/anvil-zksync/edit/main/docs/site/pages/:path',
    text: 'Suggest changes to this page',
  },
  theme: {
    // Based on Elastic Network's theme
    // Deep variable overrides
    variables: {
      // Color palette
      color: {
        // core whites & blacks
        white: { light: '#FFFFFF', dark: '#FFFFFF' },
        black: { light: '#000000', dark: '#000000' },

        // page backgrounds
        background: { light: '#FFFFFF', dark: '#000000' },
        background2: { light: '#F7F9FC', dark: '#1A1A1A' }, // subtle secondary background

        // accent usage (callouts, badges, etc.)
        backgroundAccent: { light: '#1755F4', dark: '#13D5D3' },
        backgroundAccentHover: { light: '#0C18EC', dark: '#0AAEA3' },
        backgroundAccentText: { light: '#FFFFFF', dark: '#000000' },

        // links & buttons
        link: { light: '#0C18EC', dark: '#13D5D3' },
        linkHover: { light: '#1755F4', dark: '#0AAEA3' },

        // code blocks & highlights
        codeBlockBackground: { light: '#F3F7FF', dark: '#11121A' },
        codeHighlightBackground: { light: '#E0EBFF', dark: '#121429' },
        codeHighlightBorder: { light: '#1755F4', dark: '#13D5D3' },

        // callout variants
        infoBackground: { light: '#F3F7FF', dark: '#11121A' },
        infoBorder: { light: '#0C18EC', dark: '#13D5D3' },
        infoText: { light: '#000000', dark: '#FFFFFF' },

        tipBackground: { light: '#F3F7FF', dark: '#11121A' },
        tipBorder: { light: '#0C18EC', dark: '#13D5D3' },
        tipText: { light: '#000000', dark: '#FFFFFF' },

        warningBackground: { light: '#FD402C', dark: '#2E0000' },
        warningBorder: { light: '#FD402C', dark: '#FD402C' },
        warningText: { light: '#FFFFFF', dark: '#FD402C' },

        // success/good messages
        successBackground: { light: '#BFF351', dark: '#0A2009' },
        successBorder: { light: '#BFF351', dark: '#13D5D3' },
        successText: { light: '#000000', dark: '#BFF351' },

        // tables
        tableHeaderBackground: { light: '#0C18EC', dark: '#1755F4' },
        tableHeaderText: { light: '#FFFFFF', dark: '#000000' },
      },
    },
  },
  font: {
    google: 'Inter',
  },
  iconUrl: {
    light: '/favicons/elastic_black.png',
    dark: '/favicons/elastic_white.png',
  },
  logoUrl: {
    light: '/elastic_full_black.svg',
    dark: '/elastic_full_white.svg',
  },
  rootDir: '.',
  cacheDir: resolve(process.cwd(), './.cache'),
  sidebar: [
    {
      text: 'Introduction',
      items: [
        { text: 'Why', link: '/intro' },
        { text: 'Installation', link: '/intro/installation' },
        { text: 'Getting Started', link: '/intro/getting-started' },
      ],
    },
    {
      text: 'CLI Reference',
      items: [
        { text: 'Overview', link: '/cli/' },
        { text: 'Start', link: '/cli/run' },
        { text: 'Fork', link: '/cli/fork' },
        { text: 'Replay', link: '/cli/replay_tx' },
      ],
    },
    {
      text: 'Guides',
      collapsed: true,
      items: [{ text: 'Local Hardhat Testing', link: '/guides/local_hardhat_testing' }],
    },
    {
      text: 'RPC Reference',
      items: [
        { text: 'Overview', link: '/rpc' },
        { text: 'eth_', link: '/rpc/eth' },
        { text: 'zks_', link: '/rpc/zks' },
        { text: 'anvil_', link: '/rpc/anvil' },
        { text: 'hardhat_', link: '/rpc/hardhat' },
        { text: 'misc_', link: '/rpc/misc' },
      ],
    },
    { text: 'GitHub', link: 'https://github.com/matter-labs/anvil-zksync' },
  ],
  title: 'anvil-zksync',
  topNav: [
    { link: '/cli', text: 'CLI' },
    { link: '/rpc', text: 'RPC' },
    { link: '/guides', text: 'Guides' },
  ],
  socials: [
    { icon: 'github', link: 'https://github.com/matter-labs' },
    { icon: 'x', link: 'https://x.com/zksync' },
  ],
});
