import { invoke } from '@tauri-apps/api/core';

/** A contiguous range of conditional lines allocated on a target node. */
export interface ConditionalLineRange {
  start: number;
  count: number;
}

/** A logic allocation record for one facility on one target node. */
export interface LogicAllocation {
  facilityId: string;
  targetNodeKey: string;
  conditionalLines: ConditionalLineRange;
}

/** One CDI field write produced by the compiler. */
export interface CompiledFieldWrite {
  leafPath: string;
  space: number;
  address: number;
  value: number[];
  elementType: string;
}

/** The output of the logic compiler for one facility. */
export interface CompiledLogicPlan {
  allocation: LogicAllocation;
  fieldWrites: CompiledFieldWrite[];
}

/** Capacity information for a logic target node. */
export interface LogicCapacity {
  totalLines: number;
  usedLines: number;
}

/** Compile the logic for a facility on a target node. */
export async function compileLogicForFacility(
  facilityId: string,
  targetNodeKey: string,
): Promise<CompiledLogicPlan> {
  return invoke<CompiledLogicPlan>('compile_logic_for_facility', {
    facilityId,
    targetNodeKey,
  });
}

/** Query the logic capacity of a target node. */
export async function getLogicCapacity(
  targetNodeKey: string,
): Promise<LogicCapacity> {
  return invoke<LogicCapacity>('get_logic_capacity', { targetNodeKey });
}
