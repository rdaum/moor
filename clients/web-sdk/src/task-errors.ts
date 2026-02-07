// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Lesser General Public License as published by the Free Software Foundation,
// version 3 or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Lesser General Public License for more
// details.
//
// You should have received a copy of the GNU Lesser General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

import { AbortLimitReason } from "@moor/schema/generated/moor-rpc/abort-limit-reason";
import { CommandErrorUnion } from "@moor/schema/generated/moor-rpc/command-error-union";
import { CommandExecutionError } from "@moor/schema/generated/moor-rpc/command-execution-error";
import { SchedulerError } from "@moor/schema/generated/moor-rpc/scheduler-error";
import { SchedulerErrorUnion } from "@moor/schema/generated/moor-rpc/scheduler-error-union";
import { TaskAbortedLimit } from "@moor/schema/generated/moor-rpc/task-aborted-limit";
import { VerbCompilationError } from "@moor/schema/generated/moor-rpc/verb-compilation-error";
import { VerbProgramErrorUnion } from "@moor/schema/generated/moor-rpc/verb-program-error-union";
import { VerbProgramFailed } from "@moor/schema/generated/moor-rpc/verb-program-failed";

export interface SchedulerErrorNarrative {
    message: string;
    description?: string[];
}

export function schedulerErrorToNarrative(schedulerError: SchedulerError): SchedulerErrorNarrative | null {
    const errorType = schedulerError.errorType();
    let message: string | null = null;
    let description: string[] | null = null;

    switch (errorType) {
        case SchedulerErrorUnion.CommandExecutionError: {
            const cmdExecError = schedulerError.error(new CommandExecutionError()) as CommandExecutionError | null;
            if (!cmdExecError) {
                break;
            }
            const cmdError = cmdExecError.error();
            if (!cmdError) {
                break;
            }
            switch (cmdError.errorType()) {
                case CommandErrorUnion.CouldNotParseCommand:
                    message = "I don't understand that.";
                    break;
                case CommandErrorUnion.NoObjectMatch:
                    message = "I don't see that here.";
                    break;
                case CommandErrorUnion.NoCommandMatch:
                    message = "I don't know how to do that.";
                    break;
                case CommandErrorUnion.PermissionDenied:
                    message = "You can't do that.";
                    break;
            }
            break;
        }

        case SchedulerErrorUnion.VerbProgramFailed: {
            const verbProgramFailed = schedulerError.error(new VerbProgramFailed()) as VerbProgramFailed | null;
            if (!verbProgramFailed) {
                break;
            }
            const verbError = verbProgramFailed.error();
            if (!verbError) {
                break;
            }

            switch (verbError.errorType()) {
                case VerbProgramErrorUnion.VerbCompilationError: {
                    const compError = verbError.error(new VerbCompilationError()) as VerbCompilationError | null;
                    if (compError) {
                        const ce = compError.error();
                        message = "Verb not programmed.";
                        if (ce) {
                            description = [ce.toString()];
                        }
                    }
                    break;
                }
                case VerbProgramErrorUnion.NoVerbToProgram:
                    message = "Verb not programmed.";
                    break;
                case VerbProgramErrorUnion.VerbPermissionDenied:
                    message = "Permission denied.";
                    break;
            }
            break;
        }

        case SchedulerErrorUnion.TaskAbortedLimit: {
            const abortLimit = schedulerError.error(new TaskAbortedLimit()) as TaskAbortedLimit | null;
            if (!abortLimit) {
                break;
            }
            const limit = abortLimit.limit();
            if (!limit) {
                break;
            }
            switch (limit.reason()) {
                case AbortLimitReason.Ticks:
                    message = "Task ran out of ticks";
                    break;
                case AbortLimitReason.Time:
                    message = "Task ran out of seconds";
                    break;
            }
            break;
        }

        case SchedulerErrorUnion.TaskAbortedError:
            message = "Task aborted";
            break;
        case SchedulerErrorUnion.TaskAbortedException:
            return null;
        case SchedulerErrorUnion.TaskAbortedCancelled:
            message = "Task cancelled";
            break;
        default:
            return null;
    }

    if (!message) {
        return null;
    }
    return {
        message,
        description: description ?? undefined,
    };
}
