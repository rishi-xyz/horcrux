import React from 'react';
import {useCurrentFrame} from 'remotion';
import type {Block, Line} from '../steps';
import {CHARS_PER_SEC, LINE_FRAMES} from '../steps';
import {COLORS, MONO, SANS} from '../theme';

const lineColors: Record<NonNullable<Line['color']>, string> = {
	default: COLORS.text,
	dim: COLORS.dim,
	cyan: COLORS.cyan,
	green: COLORS.green,
	red: COLORS.red,
	yellow: COLORS.yellow,
	magenta: COLORS.magenta,
};

export const Term: React.FC<{children: React.ReactNode}> = ({children}) => (
	<div
		style={{
			width: 1320,
			backgroundColor: COLORS.bgTerm,
			borderRadius: 12,
			border: `1px solid ${COLORS.border}`,
			boxShadow: '0 28px 70px rgba(0,0,0,.6)',
			overflow: 'hidden',
			fontFamily: MONO,
			fontSize: 21,
		}}
	>
		<div
			style={{
				height: 44,
				background: COLORS.headerBg,
				borderBottom: `1px solid ${COLORS.border}`,
				display: 'flex',
				alignItems: 'center',
				padding: '0 16px',
				gap: 8,
			}}
		>
			{['#ff5f56', '#ffbd2e', '#27c93f'].map((c) => (
				<div key={c} style={{width: 12, height: 12, borderRadius: 6, background: c}} />
			))}
			<div style={{flex: 1, textAlign: 'center', color: COLORS.dim, fontSize: 14}}>
				rishi@horcrux — zsh
			</div>
			<div style={{width: 52}} />
		</div>
		<div style={{padding: '26px 32px 34px', lineHeight: 1.6}}>{children}</div>
	</div>
);

export const Typed: React.FC<{text: string; start: number; frame: number}> = ({
	text,
	start,
	frame,
}) => {
	if (frame < start) return null;
	const rel = frame - start;
	const n = Math.max(0, Math.floor((rel / 30) * CHARS_PER_SEC));
	const shown = text.slice(0, n);
	const caret = n >= text.length ? (frame % 44 < 22 ? '▍' : ' ') : '▍';
	return (
		<div style={{color: COLORS.text, whiteSpace: 'pre-wrap', wordBreak: 'break-all'}}>
			<span style={{color: COLORS.green, fontWeight: 700}}>$ </span>
			{shown}
			{caret}
		</div>
	);
};

export const Lines: React.FC<{lines: Line[]; start: number; frame: number}> = ({
	lines,
	start,
	frame,
}) => (
	<>
		{lines.map((l, i) => {
			const at = start + i * LINE_FRAMES;
			if (frame < at) return null;
			const opacity = Math.min(1, (frame - at) / 6);
			return (
				<div
					key={i}
					style={{
						color: lineColors[l.color ?? 'default'],
						opacity,
						whiteSpace: 'pre-wrap',
						wordBreak: 'break-all',
					}}
				>
					{l.text === '' ? '\u00A0' : l.text}
				</div>
			);
		})}
	</>
);

export const Working: React.FC<{start: number; frames: number; frame: number}> = ({
	start,
	frames,
	frame,
}) => {
	if (frame < start || frame > start + frames) return null;
	const n = Math.floor((frame - start) / 10) % 3 + 1;
	return <div style={{color: COLORS.dim}}>{'working' + '.'.repeat(n)}</div>;
};

export const Blocks: React.FC<{blocks: Block[]}> = ({blocks}) => {
	const frame = useCurrentFrame();
	let cursor = 6;
	return (
		<>
			{blocks.map((b, i) => {
				const start = cursor;
				const typeFrames = Math.ceil(((b.cmd.length + 4) / CHARS_PER_SEC) * 30);
				const spinFrames = b.spinnerMs ? Math.ceil((b.spinnerMs / 1000) * 30) : 0;
				const lineStart = start + typeFrames + (spinFrames ? spinFrames + 6 : 8);
				cursor = lineStart + b.lines.length * LINE_FRAMES + 16;
				return (
					<React.Fragment key={i}>
						<Typed text={b.cmd} start={start} frame={frame} />
						{b.spinnerMs ? (
							<Working start={start + typeFrames} frames={spinFrames} frame={frame} />
						) : null}
						<Lines lines={b.lines} start={lineStart} frame={frame} />
					</React.Fragment>
				);
			})}
		</>
	);
};

export {SANS};
