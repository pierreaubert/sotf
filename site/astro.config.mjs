import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://sotf.spinorama.org',
  integrations: [
    starlight({
      title: 'SotF Docs',
      logo: {
        src: './public/images/sotf-icon.png',
      },
      customCss: ['./src/styles/docs.css'],
      sidebar: [
        {
          label: 'Quick Start',
          items: [
            { label: 'Play Music (TUI)', slug: 'quick-start/play-tui' },
            { label: 'Play Music (Desktop)', slug: 'quick-start/play-gpui' },
            { label: 'Better Headphones', slug: 'quick-start/headphone-quick' },
            { label: 'Better Speakers', slug: 'quick-start/speaker-quick' },
            { label: 'First Plugin', slug: 'quick-start/first-plugin' },
            { label: 'macOS System Audio', slug: 'quick-start/macos-systemwide' },
          ],
        },
        {
          label: 'Guides',
          items: [
            { label: 'Headphone EQ', slug: 'guides/headphone-eq' },
            { label: 'Speaker EQ (Spinorama)', slug: 'guides/speaker-eq' },
            { label: 'Room Correction', slug: 'guides/room-correction' },
            { label: 'Headphone Listening Chain', slug: 'guides/headphone-chain' },
            { label: 'Surround Upmixing', slug: 'guides/surround-upmix' },
            { label: 'macOS System Audio', slug: 'guides/macos-daemon' },
            { label: 'Plugin Presets', slug: 'guides/plugin-presets' },
            { label: 'Recording Measurements', slug: 'guides/recording' },
            { label: 'AutoEQ CLI', slug: 'guides/autoeq-cli' },
            { label: 'RoomEQ CLI', slug: 'guides/roomeq-cli' },
            { label: 'Listening Profiles', slug: 'guides/listening-profiles' },
          ],
        },
        {
          label: 'Reference',
          items: [
            {
              label: 'Plugins',
              autogenerate: { directory: 'reference/plugins' },
            },
            {
              label: 'Keybindings',
              autogenerate: { directory: 'reference/keybindings' },
            },
            {
              label: 'Screens',
              autogenerate: { directory: 'reference/screens' },
            },
            {
              label: 'CLI',
              autogenerate: { directory: 'reference/cli' },
            },
            {
              label: 'Configuration',
              autogenerate: { directory: 'reference/config' },
            },
          ],
        },
        {
          label: 'Concepts',
          autogenerate: { directory: 'concepts' },
        },
        {
          label: 'Troubleshooting',
          slug: 'troubleshooting',
        },
      ],
    }),
  ],
});
