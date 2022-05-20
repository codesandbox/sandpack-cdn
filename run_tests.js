const { exec, spawn } = require("node:child_process");
const path = require("node:path");
const fetch = require("node-fetch");
const urlJoin = require("url-join");

const PORT = +(process.env.PORT | "9000");

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitForServerStart(serverAddress) {
  for (let i = 0; i < 25; i++) {
    console.log("Trying to connect to the server...");
    try {
      // Fetch react-dom@18.1.0
      await fetch(
        urlJoin(serverAddress, "/package/MyhyZWFjdC1kb21AMTguMS4wKQ=="),
        {}
      );
      return true;
    } catch (err) {
      console.error(err);
    }
    console.log("Waiting 50ms before retrying...");
    await sleep(50);
  }

  throw new Error("Server did not start in time :(");
}

async function run() {
  const buildFile = path.join(__dirname, "/target/release/sandpack-cdn");
  const spawnedServer = spawn(buildFile, {
    env: {
      ...process.env,
      PORT: PORT,
    },
  });

  spawnedServer.on("spawn", () => {
    console.log("Started the server process", buildFile);
  });
  spawnedServer.on("error", (err) => console.error(err));
  spawnedServer.on("disconnect", () => console.log("Server bus disconnected"));
  spawnedServer.on("close", () => console.log("Server closed"));

  spawnedServer.stdout.setEncoding("utf-8");
  spawnedServer.stdout.on("data", (data) => {
    console.log(data);
  });
  spawnedServer.stderr.setEncoding("utf-8");
  spawnedServer.stderr.on("data", (data) => {
    console.error(data);
  });

  const cdnAddress = `http://localhost:${PORT}`;
  await waitForServerStart(cdnAddress);

  console.log("Server has responded successfully");

  const spawnedTest = spawn("jest", ["--forceExit"], {
    stdio: "inherit",
    env: {
      ...process.env,
      CDN_ROOT: cdnAddress,
    },
  });

  await new Promise((resolve, reject) => {
    spawnedTest.on("close", (code) => {
      if (code !== 0) {
        reject(new Error("tests failed"));
      } else {
        resolve();
      }
    });
  }).finally(() => {
    spawnedServer.kill();
  });
}

run().catch((err) => {
  process.exitCode = 1;
  console.error(err);
});
