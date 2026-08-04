import React from 'react';
import type {Step} from './steps';
import {StepHeader} from './components/header';
import {Blocks, Term} from './components/terminal';
import {Summary, Verify} from './components/panels';

export const StepView: React.FC<{index: number; step: Step}> = ({index, step}) => (
	<div style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 26}}>
		<StepHeader n={index + 1} title={step.title} module={step.module} note={step.note} />
		<Term>
			{step.verify ? (
				<Verify rows={step.verify} />
			) : step.summary ? (
				<Summary rows={step.summary} />
			) : (
				<Blocks blocks={step.blocks ?? []} />
			)}
		</Term>
	</div>
);
