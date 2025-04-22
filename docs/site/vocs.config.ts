// vocs.config.ts
import { defineConfig } from 'vocs'

export default defineConfig({
  sidebar: [
    {
      text: 'Introduction',
      items: [
        { text: 'Why', link: '/intro' },
        { text: 'Demo', link: '/intro/demo' },
        { text: 'Getting Started', link: '/intro/getting-started' },
        { text: 'Installation', link: '/intro/installation' },
        { text: 'Compatibility', link: '/intro/compatibility' },
      ],
    },
    {
      text: 'CLI',
      items: [
        { text: 'Start', link: '/cli/run' },
        { text: 'Fork', link: '/cli/fork' },
        { text: 'Replay', link: '/cli/replay_tx' },
      ],
    },
    {
      text: 'Guides',
      collapsed: false,
      items: [{ text: 'Getting Started', link: '/guides' }],
    },
    {
      text: 'RPC Reference',
      items: [
        { text: 'Overview', link: '/rpc' },
        { text: 'eth_', link: '/rpc/eth' },
        { text: 'zks_', link: '/rpc/zks' },
        { text: 'debug_', link: '/rpc/debug' },
        { text: 'anvil_', link: '/rpc/anvil' },
        { text: 'hardhat_', link: '/rpc/hardhat' },
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
})
