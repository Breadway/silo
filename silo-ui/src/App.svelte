<script>
  import GameList from './lib/GameList.svelte'
  import PullModal from './lib/PullModal.svelte'

  let selectedGame = null
</script>

<main>
  <header class="app-header">
    <h1>Silo</h1>
    <p class="subtitle">Game archive catalog</p>
  </header>

  <GameList onSelect={(g) => selectedGame = g} />

  {#if selectedGame}
    <PullModal game={selectedGame} onClose={() => selectedGame = null} />
  {/if}
</main>

<style>
  :global(*, *::before, *::after) { box-sizing: border-box; }
  :global(body) {
    margin: 0;
    background: #1e1e2e;
    color: #cdd6f4;
    font-family: system-ui, -apple-system, sans-serif;
  }

  main {
    max-width: 900px;
    margin: 0 auto;
    padding: 32px 16px;
  }

  .app-header {
    margin-bottom: 28px;
  }

  h1 {
    margin: 0;
    font-size: 1.8rem;
    font-weight: 700;
    color: #89b4fa;
    letter-spacing: -0.02em;
  }

  .subtitle {
    margin: 4px 0 0;
    color: #585b70;
    font-size: .9rem;
  }
</style>
