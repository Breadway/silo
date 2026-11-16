<script>
  import { startPull, subscribePull } from './api.js'

  export let game
  export let onClose

  let events = []
  let done = false
  let error = false
  let runGameId = null
  let pulling = false

  async function pull() {
    pulling = true
    events = []
    done = false
    error = false
    runGameId = null

    let jobId
    try {
      const res = await startPull(game.id)
      jobId = res.job_id
    } catch (e) {
      events = [{ stage: 'error', message: String(e) }]
      error = true
      pulling = false
      return
    }

    const es = subscribePull(jobId, (ev) => {
      events = [...events, ev]
      if (ev.stage === 'done') {
        done = true
        pulling = false
        runGameId = ev.run_game_id
        es.close()
      } else if (ev.stage === 'error') {
        error = true
        pulling = false
        es.close()
      }
    })

    es.onerror = () => {
      if (!done && !error) {
        events = [...events, { stage: 'error', message: 'Connection lost' }]
        error = true
        pulling = false
      }
      es.close()
    }
  }
</script>

<div class="modal-backdrop" on:click|self={onClose}>
  <div class="modal">
    <header>
      <h2>{game.name}</h2>
      <button class="close" on:click={onClose}>✕</button>
    </header>

    <div class="size">
      {(game.size_bytes / 1_073_741_824).toFixed(2)} GB
    </div>

    {#if game.notes}
      <p class="notes">{game.notes}</p>
    {/if}

    {#if game.pulls?.length}
      <div class="pulls">
        Previously pulled on: {game.pulls.map(p => `${p.machine_name} (${new Date(p.pulled_at).toLocaleDateString()})`).join(', ')}
      </div>
    {/if}

    {#if events.length}
      <div class="log">
        {#each events as ev}
          <div class="log-line" class:err={ev.stage === 'error'} class:ok={ev.stage === 'done'}>
            <span class="stage">[{ev.stage}]</span> {ev.message}
          </div>
        {/each}
      </div>
    {/if}

    {#if done && runGameId}
      <a class="steam-link" href="steam://rungameid/{runGameId}">
        Launch in Steam
      </a>
    {/if}

    <footer>
      {#if !pulling && !done}
        <button class="primary" on:click={pull}>Pull &amp; Install</button>
      {:else if pulling}
        <button disabled>Pulling…</button>
      {/if}
      <button on:click={onClose}>Close</button>
    </footer>
  </div>
</div>

<style>
  .modal-backdrop {
    position: fixed; inset: 0;
    background: rgba(0,0,0,.55);
    display: flex; align-items: center; justify-content: center;
    z-index: 100;
  }
  .modal {
    background: #1e1e2e; color: #cdd6f4;
    border-radius: 8px; padding: 24px;
    width: min(540px, 92vw);
    max-height: 80vh; overflow-y: auto;
    display: flex; flex-direction: column; gap: 12px;
  }
  header { display: flex; justify-content: space-between; align-items: center; }
  h2 { margin: 0; font-size: 1.2rem; }
  .close { background: none; border: none; color: #cdd6f4; cursor: pointer; font-size: 1.1rem; }
  .size { color: #a6adc8; font-size: .85rem; }
  .notes { color: #a6adc8; font-size: .9rem; margin: 0; }
  .pulls { font-size: .8rem; color: #585b70; }
  .log {
    background: #11111b; border-radius: 4px; padding: 10px;
    font-family: monospace; font-size: .8rem;
    max-height: 240px; overflow-y: auto;
    display: flex; flex-direction: column; gap: 4px;
  }
  .log-line { color: #cdd6f4; }
  .log-line.err { color: #f38ba8; }
  .log-line.ok { color: #a6e3a1; }
  .stage { color: #89b4fa; }
  .steam-link {
    display: inline-block; background: #1b2838; color: #c7d5e0;
    padding: 8px 16px; border-radius: 4px; text-decoration: none;
    font-size: .9rem; align-self: flex-start;
  }
  footer { display: flex; gap: 8px; justify-content: flex-end; }
  button { padding: 8px 16px; border-radius: 4px; border: none; cursor: pointer; }
  button.primary { background: #89b4fa; color: #1e1e2e; font-weight: 600; }
  button:disabled { opacity: .5; cursor: default; }
  button:not(.primary):not(:disabled) { background: #313244; color: #cdd6f4; }
</style>
