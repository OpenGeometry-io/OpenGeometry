import { errorMessage, CliUsageError } from "./argv.js";
import { runAddCommand } from "../commands/add-command.js";
import { runProjectCommand } from "../commands/project-command.js";
import { runSceneCommand } from "../commands/scene-command.js";

const HELP_TEXT = `OpenGeometry CLI (bootstrap)\n\nUsage:\n  opengeometry scene create <name>\n  opengeometry scene list\n  opengeometry scene use <sceneId>\n  opengeometry scene show [sceneId]\n\n  opengeometry add line --start x,y,z --end x,y,z [--id <entityId>] [--scene <sceneId>]\n\n  opengeometry project 2d [--scene <sceneId>] [--camera-json <path>] [--hlr-json <path>] [--pretty]\n`;

export async function runCli(argv: string[], cwd = process.cwd()): Promise<number> {
  if (argv.length === 0 || argv[0] === "help" || argv[0] === "--help") {
    process.stdout.write(`${HELP_TEXT}\n`);
    return 0;
  }

  const [group, ...rest] = argv;

  try {
    let output: string;

    switch (group) {
      case "scene":
        output = await runSceneCommand(rest, cwd);
        break;
      case "add":
        output = await runAddCommand(rest, cwd);
        break;
      case "project":
        output = await runProjectCommand(rest, cwd);
        break;
      default:
        throw new CliUsageError(`Unknown command group '${group}'. Run 'opengeometry --help'.`);
    }

    process.stdout.write(`${output}\n`);
    return 0;
  } catch (error) {
    const prefix = error instanceof CliUsageError ? "Usage error" : "Command failed";
    process.stderr.write(`${prefix}: ${errorMessage(error)}\n`);
    return 1;
  }
}
