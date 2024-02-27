const { spawn, spawnSync } = require("child_process");
const fs = require("fs");

let electron;
let is_electron = false;
try {
    electron = require('electron');
    is_electron = true;
} catch { }

function emitLines(stream) {
    var backlog = "";
    stream.on("data", function (data) {
        backlog += data
        var n = backlog.indexOf('\n')
        // got a \n? emit one or more 'line' events
        while (~n) {
            stream.emit("line", backlog.substring(0, n))
            backlog = backlog.substring(n + 1)
            n = backlog.indexOf('\n')
        }
    })
    stream.on("end", function () {
        if (backlog) {
            stream.emit("line", backlog)
        }
    })
}

function nativeDoesFileExist(path: string): boolean {
    return fs.existsSync(path);
}

function nativeCurrentProcessId(): number {
    if (is_electron && !electron.app) {
        return electron.ipcRenderer.sendSync('velopack-get-pid');
    } else {
        return process.pid;
    }
}

function nativeGetCurrentProcessPath(): string {
    return process.execPath;
}

function nativeCurrentOsName(): string {
    return process.platform;
}

function nativeExitProcess(code: number): void {
    if (is_electron) {
        if (electron.app) {
            electron.app.quit(code);
            process.exit(code);
        } else if (electron.remote) {
            electron.remote.app.quit(code);
        } else if (electron.ipcRenderer) {
            electron.ipcRenderer.send('velopack-quit', code);
        } else {
            throw new Error('Could not find a way to exit the process, electron.app, electron.remote.app, and electron.ipcRenderer are all undefined.');
        }
    } else {
        process.exit(code);
    }
}

function nativeRegisterElectron(): void {
    if (is_electron) {
        electron.ipcMain.on('velopack-quit', (event, code) => {
            electron.app.quit(code);
        });
        electron.ipcMain.on('velopack-get-pid', (event) => {
            event.returnValue = process.pid;
        });
        electron.ipcMain.on('velopack-exec-fire-forget', (event, command) => {
            nativeStartProcessFireAndForget(command);
        });
        electron.ipcMain.on('velopack-exec-blocking', (event, command) => {
            event.returnValue = nativeStartProcessBlocking(command);
        });
    }
}

function nativeStartProcessFireAndForget(command_line: readonly string[]): void {
    if (is_electron && !electron.app) {
        electron.ipcRenderer.send('velopack-exec-fire-forget', command_line);
    } else {
        spawn(command_line[0], command_line.slice(1), { encoding: "utf8" });
    }
}

function nativeStartProcessBlocking(command_line: readonly string[]): string {
    if (is_electron && !electron.app) {
        return electron.ipcRenderer.sendSync('velopack-exec-blocking', command_line);
    } else {
        const child = spawnSync(command_line[0], command_line.slice(1), { encoding: "utf8" });
        if (child.status !== 0) {
            throw new Error(`Process returned non-zero exit code (${child.status}). Check the log for more details.`);
        }
        return child.stdout;
    }
}

function nativeStartProcessAsync(command_line: readonly string[]): Promise<string> {
    return new Promise((resolve, reject) => {
        const process = spawnSync(command_line[0], command_line.slice(1), { encoding: "utf8" });

        let output = '';
        process.stdout.on('data', (data) => {
            output += data.toString();
        });

        process.stderr.on('data', (data) => {
            console.error(`stderr: ${data}`);
        });

        process.on('close', (code) => {
            if (code === 0) {
                resolve(output.trim());
            } else {
                reject(new Error(`Process exited with code: ${code}`));
            }
        });

        process.on('error', (err) => {
            reject(err);
        });
    });
}

function nativeStartProcessAsyncReadLine(command_line: readonly string[], handler: Function): Promise<void> {
    return new Promise((resolve, reject) => {
        const child = spawn(command_line[0], command_line.slice(1), { encoding: "utf8" });

        // Emitting lines for each stdout data event
        emitLines(child.stdout);

        child.stdout.resume();
        child.stdout.setEncoding("utf8");
        child.stdout.on("line", (data) => {
            handler(data);
        });

        // Handling the process exit
        child.on('exit', (code) => {
            if (code === 0) {
                resolve(); // Process completed successfully
            } else {
                reject(new Error(`Process exited with code: ${code}`)); // Process failed
            }
        });

        // Handling process errors (e.g., if the process could not be spawned, killed or sending a message to it fails)
        child.on('error', (err) => {
            reject(err); // Process encountered an error
        });
    });
}

