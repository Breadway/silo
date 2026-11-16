<script>
  export let game
  export let onSelect

  $: lastPull = game.pulls?.[0]
  $: gb = (game.size_bytes / 1_073_741_824).toFixed(1)
</script>

<button class="card" on:click={() => onSelect(game)}>
  <div class="name">{game.name}</div>
  <div class="meta">
    <span class="size">{gb} GB</span>
    {#if lastPull}
      <span class="pulled">
        Last pulled: {lastPull.machine_name}
        ({new Date(lastPull.pulled_at).toLocaleDateString()})
      </span>
    {:else}
      <span class="never">Never pulled</span>
    {/if}
  </div>
  {#if game.notes}
    <div class="notes">{game.notes}</div>
  {/if}
</button>

<style>
  .card {
    background: #181825; border: 1px solid #313244;
    border-radius: 6px; padding: 14px 16px;
    text-align: left; cursor: pointer; width: 100%;
    display: flex; flex-direction: column; gap: 6px;
    transition: border-color .15s;
  }
  .card:hover { border-color: #89b4fa; }
  .name { font-size: 1rem; font-weight: 600; color: #cdd6f4; }
  .meta { display: flex; gap: 16px; flex-wrap: wrap; }
  .size { color: #89b4fa; font-size: .85rem; }
  .pulled { color: #a6adc8; font-size: .8rem; }
  .never { color: #585b70; font-size: .8rem; }
  .notes { color: #a6adc8; font-size: .82rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
