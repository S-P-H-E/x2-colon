<script lang="ts">
	import './globals.css';
	import favicon from '$lib/assets/favicon.ico';

	let { children } = $props();
</script>

<svelte:head>
	<title>Empty</title>
	<meta name="description" content="This is an empty project" />
	<link rel="icon" href={favicon} />
</svelte:head>
{@render children()}
