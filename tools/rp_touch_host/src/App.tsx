import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import * as THREE from "three";
import { GLTFLoader, type GLTF } from "three/examples/jsm/loaders/GLTFLoader.js";
import "./App.css";

type ModelPayload = {
  gltf: string;
};

type TelemetryAnglePayload = {
  pitchDeg: number;
  rollDeg: number;
  quatW: number;
  quatX: number;
  quatY: number;
  quatZ: number;
};

type SerialConnectionPayload = {
  connected: boolean;
  portName: string | null;
};

const toRadians = (value: number) => (value * Math.PI) / 180;
const PITCH_BASELINE_DEG = -20;
const X_AXIS = new THREE.Vector3(1, 0, 0);
const BASELINE_QUAT = new THREE.Quaternion().setFromAxisAngle(
  X_AXIS,
  toRadians(-PITCH_BASELINE_DEG)
);

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function sanitizeTelemetryPayload(
  raw: Partial<TelemetryAnglePayload>,
  fallback: TelemetryAnglePayload
): TelemetryAnglePayload {
  const pitchDeg = isFiniteNumber(raw.pitchDeg) ? raw.pitchDeg : fallback.pitchDeg;
  const rollDeg = isFiniteNumber(raw.rollDeg) ? raw.rollDeg : fallback.rollDeg;
  const quatW = isFiniteNumber(raw.quatW) ? raw.quatW : fallback.quatW;
  const quatX = isFiniteNumber(raw.quatX) ? raw.quatX : fallback.quatX;
  const quatY = isFiniteNumber(raw.quatY) ? raw.quatY : fallback.quatY;
  const quatZ = isFiniteNumber(raw.quatZ) ? raw.quatZ : fallback.quatZ;
  return { pitchDeg, rollDeg, quatW, quatX, quatY, quatZ };
}

function toViewerQuaternion(sample: TelemetryAnglePayload): THREE.Quaternion {
  const sensorQuat = new THREE.Quaternion(
    sample.quatX,
    sample.quatY,
    sample.quatZ,
    sample.quatW
  ).normalize();
  const modelQuat = sensorQuat.clone().conjugate();
  return BASELINE_QUAT.clone().multiply(modelQuat);
}

function App() {
  const mountRef = useRef<HTMLDivElement | null>(null);
  const pivotRef = useRef<THREE.Group | null>(null);

  const [model, setModel] = useState<ModelPayload | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [viewerError, setViewerError] = useState<string | null>(null);

  const [ports, setPorts] = useState<string[]>([]);
  const [selectedPort, setSelectedPort] = useState("");
  const [serialConnected, setSerialConnected] = useState(false);
  const [serialBusy, setSerialBusy] = useState(false);
  const [serialError, setSerialError] = useState<string | null>(null);

  const [orientation, setOrientation] = useState<TelemetryAnglePayload>({
    pitchDeg: 0,
    rollDeg: 0,
    quatW: 1,
    quatX: 0,
    quatY: 0,
    quatZ: 0,
  });

  useEffect(() => {
    let isCancelled = false;
    let unlisten: UnlistenFn | null = null;

    const init = async () => {
      setIsLoading(true);
      setLoadError(null);

      try {
        const payload = await invoke<ModelPayload>("load_rp_touch_model");
        if (!isCancelled) {
          setModel(payload);
        }
      } catch (error) {
        if (!isCancelled) {
          const message = error instanceof Error ? error.message : String(error);
          setLoadError(message);
        }
      } finally {
        if (!isCancelled) {
          setIsLoading(false);
        }
      }

      try {
        const serialPorts = await invoke<string[]>("list_serial_ports");
        if (!isCancelled) {
          setPorts(serialPorts);
          setSelectedPort((prev) =>
            serialPorts.includes(prev) ? prev : (serialPorts[0] ?? "")
          );
        }
      } catch {
        if (!isCancelled) {
          setPorts([]);
          setSelectedPort("");
        }
      }

      try {
        const state = await invoke<SerialConnectionPayload>(
          "serial_connection_state"
        );
        if (!isCancelled) {
          setSerialConnected(state.connected);
          if (state.portName) {
            setSelectedPort(state.portName);
          }
        }
      } catch {
        if (!isCancelled) {
          setSerialConnected(false);
        }
      }

      unlisten = await listen<TelemetryAnglePayload>(
        "telemetry-angle",
        (event) => {
          if (isCancelled) {
            return;
          }
          setOrientation((prev) =>
            sanitizeTelemetryPayload(
              event.payload as Partial<TelemetryAnglePayload>,
              prev
            )
          );
        }
      );
    };

    void init();

    return () => {
      isCancelled = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  useEffect(() => {
    const pivot = pivotRef.current;
    if (!pivot) {
      return;
    }

    pivot.quaternion.copy(toViewerQuaternion(orientation));
  }, [orientation]);

  useEffect(() => {
    const container = mountRef.current;
    if (!container || !model) {
      return;
    }

    let disposed = false;
    let animationId = 0;
    setViewerError(null);

    const scene = new THREE.Scene();
    scene.background = new THREE.Color("#e9eff6");

    const camera = new THREE.PerspectiveCamera(45, 1, 0.001, 100);
    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));

    container.innerHTML = "";
    container.appendChild(renderer.domElement);

    const hemisphereLight = new THREE.HemisphereLight(0xffffff, 0x3f4e67, 1.2);
    const keyLight = new THREE.DirectionalLight(0xffffff, 1.1);
    keyLight.position.set(2.2, 2.8, 1.6);
    const fillLight = new THREE.DirectionalLight(0x9eb3d6, 0.65);
    fillLight.position.set(-2.4, 1.2, -1.8);

    scene.add(hemisphereLight);
    scene.add(keyLight);
    scene.add(fillLight);

    const pivot = new THREE.Group();
    pivot.quaternion.copy(toViewerQuaternion(orientation));
    scene.add(pivot);
    pivotRef.current = pivot;

    const resize = () => {
      const width = Math.max(container.clientWidth, 1);
      const height = Math.max(container.clientHeight, 1);
      camera.aspect = width / height;
      camera.updateProjectionMatrix();
      renderer.setSize(width, height, false);
    };

    const observer = new ResizeObserver(resize);
    observer.observe(container);
    resize();

    const loader = new GLTFLoader();
    loader.parse(
      model.gltf,
      "",
      (gltf: GLTF) => {
        if (disposed) {
          return;
        }

        const root = gltf.scene;
        const box = new THREE.Box3().setFromObject(root);
        const center = box.getCenter(new THREE.Vector3());
        const size = box.getSize(new THREE.Vector3());
        const maxDim = Math.max(size.x, size.y, size.z, 0.01);

        root.position.sub(center);
        pivot.add(root);

        const distance = maxDim * 2.6;
        camera.near = Math.max(maxDim / 100, 0.0001);
        camera.far = Math.max(maxDim * 100, 10);
        camera.position.set(0, 0, distance);
        camera.lookAt(0, 0, 0);
        camera.updateProjectionMatrix();
      },
      (error: unknown) => {
        if (disposed) {
          return;
        }
        const message = error instanceof Error ? error.message : String(error);
        setViewerError(message);
      }
    );

    const animate = () => {
      if (disposed) {
        return;
      }
      renderer.render(scene, camera);
      animationId = requestAnimationFrame(animate);
    };
    animate();

    return () => {
      disposed = true;
      pivotRef.current = null;
      cancelAnimationFrame(animationId);
      observer.disconnect();
      renderer.dispose();
      scene.clear();
      container.innerHTML = "";
    };
  }, [model]);

  const refreshPorts = async () => {
    setSerialError(null);
    try {
      const serialPorts = await invoke<string[]>("list_serial_ports");
      setPorts(serialPorts);
      setSelectedPort((prev) =>
        serialPorts.includes(prev) ? prev : (serialPorts[0] ?? "")
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setSerialError(message);
    }
  };

  const connectSerial = async () => {
    setSerialBusy(true);
    setSerialError(null);

    try {
      const connectedPort = await invoke<string>("connect_serial", {
        port: selectedPort || null,
      });
      setSerialConnected(true);
      setSelectedPort(connectedPort);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setSerialError(message);
      setSerialConnected(false);
    } finally {
      setSerialBusy(false);
    }
  };

  const disconnectSerial = async () => {
    setSerialBusy(true);
    setSerialError(null);

    try {
      await invoke("disconnect_serial");
      setSerialConnected(false);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setSerialError(message);
    } finally {
      setSerialBusy(false);
    }
  };

  const resetHeading = async () => {
    setSerialError(null);
    try {
      await invoke("reset_heading");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setSerialError(message);
    }
  };

  return (
    <main className="app-shell">
      <section className="viewport">
        <div className="serial-overlay">
          <label className="serial-control">
            <span>Serial</span>
            <select
              value={selectedPort}
              disabled={serialConnected || serialBusy || ports.length === 0}
              onChange={(event) => setSelectedPort(event.currentTarget.value)}
            >
              {ports.length === 0 && <option value="">No Ports</option>}
              {ports.map((port) => (
                <option key={port} value={port}>
                  {port}
                </option>
              ))}
            </select>
          </label>

          <div className="serial-actions">
            <button
              type="button"
              className="toolbar-button"
              onClick={refreshPorts}
              disabled={serialBusy}
            >
              Refresh
            </button>
            {!serialConnected ? (
              <button
                type="button"
                className="toolbar-button"
                onClick={connectSerial}
                disabled={serialBusy || ports.length === 0}
              >
                Connect
              </button>
            ) : (
              <button
                type="button"
                className="toolbar-button"
                onClick={disconnectSerial}
                disabled={serialBusy}
              >
                Disconnect
              </button>
            )}
          </div>
        </div>

        <button
          type="button"
          className="yaw-reset-button"
          onClick={resetHeading}
          disabled={serialBusy || !serialConnected}
        >
          Reset Yaw
        </button>

        {isLoading && <div className="status">Loading model...</div>}
        {loadError && <div className="status error">{loadError}</div>}
        {!isLoading && !loadError && viewerError && (
          <div className="status error">{viewerError}</div>
        )}
        {!isLoading && !loadError && !viewerError && serialError && (
          <div className="status error">{serialError}</div>
        )}
        <div
          ref={mountRef}
          className={`viewer-canvas ${!isLoading && !loadError ? "ready" : ""}`}
        />
      </section>
    </main>
  );
}

export default App;
