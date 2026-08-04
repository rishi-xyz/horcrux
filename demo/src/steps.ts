// Authentic output captured from a real `demo.sh` run (see repo demo.sh),
// re-enacted as a synthetic terminal. Paths are shown relative (`shards/`)
// instead of the temp dir used live.

export const FPS = 30;
export const CHARS_PER_SEC = 34;
export const LINE_FRAMES = 18;
export const INTRO_FRAMES = 100;

export const FULL_KEY =
	'0x458c749b055b26600626a2d537f147766fb1cb83352a46023d05755dc6597242';

export const PW = '••••••••••••••••';

export type Color = 'default' | 'dim' | 'cyan' | 'green' | 'red' | 'yellow' | 'magenta';

export interface Line {
	text: string;
	color?: Color;
}

export interface Block {
	cmd: string;
	lines: Line[];
	/** KDF wait in ms (Argon2id work) shown as a "working…" spinner. */
	spinnerMs?: number;
}

export interface VerifyRow {
	label: string;
	key: string;
}

export interface Step {
	title: string;
	module: string;
	note: string;
	blocks?: Block[];
	verify?: VerifyRow[];
	summary?: string[];
}

const rec = (n: string) => `Reconstructed key: 0x${FULL_KEY.slice(2)}`;

const steps: Step[] = [
	{
		title: 'Build the binary',
		module: 'cargo build',
		note: 'Compiles the whole crate: CLI (src/main.rs), pipeline (src/lib.rs), and the sss / crypto / shard / error modules.',
		blocks: [
			{
				cmd: 'cargo build --release',
				lines: [
					{text: '   Compiling horcrux v0.1.0', color: 'dim'},
					{text: '    Finished `release` profile [optimized] target(s) in 8.2s', color: 'green'},
				],
			},
		],
	},
	{
		title: 'init — split the key, encrypt each shard',
		module: 'lib.rs · sss.rs · crypto.rs · shard.rs',
		note: 'Shamir split via vsss-rs over the secp256k1 field, then each share is sealed with Argon2id + AES-256-GCM under its own guardian password.',
		blocks: [
			{
				cmd: `horcrux init --generate --threshold 2 --shares 3 --out-dir shards --password '${PW}'`,
				spinnerMs: 3200,
				lines: [
					{text: `Generated test key: ${FULL_KEY}`, color: 'green'},
					{text: 'Wrote 3 shards to shards (threshold 2):', color: 'default'},
					{text: '  shards/shard-1.hx', color: 'dim'},
					{text: '  shards/shard-2.hx', color: 'dim'},
					{text: '  shards/shard-3.hx', color: 'dim'},
				],
			},
		],
	},
	{
		title: 'Inspect the shard files',
		module: 'shard.rs — SHARD_LEN = 83 bytes',
		note: 'Every shard is a fixed 83-byte binary file. The first 7 bytes are the header; the rest is salt, nonce, ciphertext and auth tag.',
		blocks: [
			{
				cmd: 'ls -la shards',
				lines: [
					{text: 'total 12', color: 'dim'},
					{text: '-rw-r--r-- 1 rishi rishi 83 shard-1.hx', color: 'default'},
					{text: '-rw-r--r-- 1 rishi rishi 83 shard-2.hx', color: 'default'},
					{text: '-rw-r--r-- 1 rishi rishi 83 shard-3.hx', color: 'default'},
				],
			},
			{
				cmd: 'hexdump -C shards/shard-1.hx',
				lines: [
					{text: '00000000  48 58 31 01 02 03 01 4a  a6 fc 56 2f 5b d2 c9 13  |HX1....J..V/[...|', color: 'dim'},
					{text: '00000010  88 b6 5e ef 11 3e a5 3f  82 c2 0a 07 0f 39 e1 ce  |..^..>.?.....9..|', color: 'dim'},
					{text: '00000020  cd ea 8c e1 81 bc e1 82  74 50 13 2e 0e d1 f9 c8  |........tP......|', color: 'dim'},
					{text: '00000030  05 8d 5e 25 a6 bb 07 ee  c1 be df 29 18 a9 40 a5  |..^%.......)..@.|', color: 'dim'},
					{text: '00000040  06 4a 9c 05 c5 02 a3 44  84 c0 6b 4b 52 44 1a a1  |.J.....D..kKRD..|', color: 'dim'},
					{text: '00000050  d6 ee cd                                          |...|             ', color: 'dim'},
					{text: '00000053', color: 'dim'},
					{text: '', color: 'dim'},
					{text: 'offset  len  field', color: 'cyan'},
					{text: '0       3    magic "HX1"', color: 'cyan'},
					{text: '3       1    format version = 1', color: 'cyan'},
					{text: '4       1    threshold t = 2', color: 'cyan'},
					{text: '5       1    share count n = 3', color: 'cyan'},
					{text: '6       1    share id = 1', color: 'cyan'},
					{text: '7       16   Argon2id salt (random per shard)', color: 'cyan'},
					{text: '23      12   AES-256-GCM nonce (random per shard)', color: 'cyan'},
					{text: '35      32   sealed share value (ciphertext)', color: 'cyan'},
					{text: '67      16   GCM authentication tag', color: 'cyan'},
					{text: '83      --   total file size', color: 'cyan'},
				],
			},
		],
	},
	{
		title: 'Reconstruct — shards 1 and 2',
		module: 'lib.rs · shard.rs · sss.rs',
		note: 'reconstruct re-derives each AES key, decrypts through the GCM tag check, then recovers the key via Lagrange interpolation.',
		blocks: [
			{
				cmd: `horcrux reconstruct shards/shard-1.hx shards/shard-2.hx --password '${PW}'`,
				spinnerMs: 2500,
				lines: [{text: rec('12'), color: 'green'}],
			},
		],
	},
	{
		title: 'Reconstruct — a different pair',
		module: 'sss.rs — any threshold subset',
		note: 'Any 2 of 3 must recover the same key. Here the pair is shards 2 and 3.',
		blocks: [
			{
				cmd: `horcrux reconstruct shards/shard-2.hx shards/shard-3.hx --password '${PW}'`,
				spinnerMs: 2500,
				lines: [{text: rec('23'), color: 'green'}],
			},
		],
	},
	{
		title: 'Reconstruct — all three shards',
		module: 'sss.rs — more than t works',
		note: 'More than the threshold is fine too: the polynomial still interpolates to the same constant term.',
		blocks: [
			{
				cmd: `horcrux reconstruct shards/shard-1.hx shards/shard-2.hx shards/shard-3.hx --password '${PW}'`,
				spinnerMs: 3000,
				lines: [{text: rec('123'), color: 'green'}],
			},
		],
	},
	{
		title: 'Verify — every reconstruction matches',
		module: 'round-trip guarantee',
		note: 'All three subsets reproduce the exact original private key.',
		verify: [
			{label: 'shards 1+2  ', key: FULL_KEY},
			{label: 'shards 2+3  ', key: FULL_KEY},
			{label: 'shards 1+2+3', key: FULL_KEY},
		],
	},
	{
		title: 'Wrong password — clean rejection',
		module: 'crypto.rs — GCM tag → Error::Decrypt',
		note: 'AES-256-GCM authenticates everything. A wrong password fails the tag: no plaintext is ever produced.',
		blocks: [
			{
				cmd: `horcrux reconstruct shards/shard-1.hx shards/shard-2.hx --password 'not-the-password'`,
				spinnerMs: 2500,
				lines: [
					{
						text: 'Error: failed to decrypt shard 1: authenticated encryption failed: wrong password or tampered shard',
						color: 'red',
					},
				],
			},
		],
	},
	{
		title: 'Too few shards — NotEnoughShares',
		module: 'lib.rs — Error::NotEnoughShares(2, 1)',
		note: 'With threshold 2, a single shard cannot interpolate the polynomial. reconstruct rejects it before any decryption.',
		blocks: [
			{
				cmd: `horcrux reconstruct shards/shard-1.hx --password '${PW}'`,
				lines: [{text: 'Error: need 2 shards but only 1 were provided', color: 'red'}],
			},
		],
	},
	{
		title: 'Mixed splits — SplitMismatch',
		module: 'shard.rs — AAD [t, n, id] binding',
		note: 'Each shard is bound to its split. A 3-of-3 split is created; combining one of its shards with the 2-of-3 split is rejected.',
		blocks: [
			{
				cmd: `horcrux reconstruct shards/shard-1.hx shards-b/shard-1.hx --password '${PW}'`,
				lines: [
					{
						text: 'Error: shard "shards-b/shard-1.hx" has different split parameters (t=3, n=3)',
						color: 'red',
					},
				],
			},
		],
	},
	{
		title: 'Summary',
		module: 'Phase 1 complete',
		note: 'Everything built so far, end to end.',
		summary: [
			'[ OK ] split:      2-of-3 encrypted shard files (83 B each)',
			'[ OK ] inspect:    83-byte format (magic / version / t / n / id / salt / nonce / ciphertext)',
			'[ OK ] reconstruct: any 2 of 3 recover the original key',
			'[ OK ] reconstruct: all 3 shards also recover the key',
			'[ OK ] rejected:    wrong password (AES-GCM auth-tag failure)',
			'[ OK ] rejected:    too few shards (NotEnoughShares)',
			'[ OK ] rejected:    mixed splits (SplitMismatch)',
			'[ OK ] tests:       23 unit + 8 integration tests green',
		],
	},
];

function blockFrames(b: Block): number {
	let f = 0;
	f += Math.ceil(((b.cmd.length + 4) / CHARS_PER_SEC) * FPS);
	if (b.spinnerMs) {
		f += Math.ceil((b.spinnerMs / 1000) * FPS);
	}
	f += 10;
	f += b.lines.length * LINE_FRAMES;
	f += 16;
	return f;
}

function stepFrames(s: Step): number {
	const head = 24;
	if (s.verify) return head + 30 + s.verify.length * 20 + 40;
	if (s.summary) return head + 30 + s.summary.length * 20 + 40;
	return head + (s.blocks ?? []).reduce((acc, b) => acc + blockFrames(b), 0) + 20;
}

export interface Scheduled {
	step: Step;
	start: number;
	len: number;
}

export function schedule(): Scheduled[] {
	let cursor = INTRO_FRAMES;
	return steps.map((step) => {
		const len = stepFrames(step);
		const out = {step, start: cursor, len};
		cursor += len;
		return out;
	});
}

export function computeTotalFrames(): number {
	const sched = schedule();
	const last = sched[sched.length - 1];
	return last.start + last.len;
}
