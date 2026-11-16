<script>
  import { listGames } from './api.js'
  import GameCard from './GameCard.svelte'

  export let onSelect

  let games = []
  let loading = true
  let error = null
  let query = ''

  async function load() {
    loading = true
    error = null
    try {
      games = await listGames()
    } catch (e) {
      error = String(e)
    } finally {
      loading = false
    }
  }

  load()

  $: filtered = games.filter(g =>
    g.name.toLowerCase().includes(query.toLowerCase()) ||
    (g.notes ?? '').toLowerCase().includes(query.toLowerCase())
  )
</script>

<div class="list-root">
  <div class="toolbar">
    <input
      class="search"
      type="search"
      placeholder="Search…"
      bind:value={query}
    />
    <button class="refresh" on:click={load} title="Refresh">↻</button>
  </div>

  {#if loading}
    <div class="status">Loading catalog…</div>
  {:else if error}
    <div class="status err">
      Failed to load catalog: {error}
      <br/>
      <small>Is silo-server running and SILO_SERVER_URL set?</small>
    </div>
  {:else if filtered.length === 0}
    <div class="status">
      {query ? 'No games match that search.' : 'No games in catalog yet.'}
    </div>
  {:else}
    <div class="grid">
      {#each filtered as game (game.id)}
        <GameCard {game} onSelect={onSelect} />
      {/each}
    </div>
  {/if}
</div>

<style>
  .list-root { display: flex; flex-direction: column; gap: 16px; }
  .toolbar { display: flex; gap: 8px; }
  .search {
    flex: 1; background: #181825; border: 1px solid #313244;
    border-radius: 4px; padding: 8px 12px;
    color: #cdd6f4; font-size: .9rem; outline: none;
  }
  .search:focus { border-color: #89b4fa; }
  .refresh {
    background: #313244; border: none; color: #cdd6f4;
    border-radius: 4px; padding: 8px 12px; cursor: pointer;
    font-size: 1rem;
  }
  .refresh:hover { background: #45475a; }
  .status { color: #a6adc8; padding: 24px 0; text-align: center; }
  .status.err { color: #f38ba8; }
  .grid { display: flex; flex-direction: column; gap: 8px; }
</style>
