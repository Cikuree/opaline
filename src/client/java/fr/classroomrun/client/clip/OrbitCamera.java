package fr.classroomrun.client.clip;

/** Orbit camera math: a point at {@code distance} from a target, looking at it. Outputs are public fields to avoid allocation. */
public final class OrbitCamera {
	public static final float MIN_DISTANCE = 3f;
	public static final float MAX_DISTANCE = 20f;
	private static final float DRAG_DEGREES_PER_PIXEL = 0.4f;
	private static final float MAX_PITCH = 85f;

	/** Camera position, and the Minecraft yaw/pitch (degrees) it looks with. Valid after {@link #update}. */
	public double x, y, z;
	public float yRot, xRot;

	private float yaw;
	private float pitch = 25f;
	private float distance = 8f;

	public OrbitCamera(float startYaw) {
		this.yaw = startYaw;
	}

	public void rotate(double dragX, double dragY) {
		this.yaw += (float) dragX * DRAG_DEGREES_PER_PIXEL;
		this.pitch = Math.max(-MAX_PITCH, Math.min(MAX_PITCH, this.pitch + (float) dragY * DRAG_DEGREES_PER_PIXEL));
	}

	/** One scroll notch = one block. */
	public void zoom(double scroll) {
		this.distance = (float) Math.max(MIN_DISTANCE, Math.min(MAX_DISTANCE, this.distance - scroll));
	}

	public float distance() {
		return this.distance;
	}

	public void update(double targetX, double targetY, double targetZ) {
		double yr = Math.toRadians(this.yaw);
		double xr = Math.toRadians(this.pitch);
		double cos = Math.cos(xr);
		// Minecraft look vector: (-sin(yaw) cos(pitch), -sin(pitch), cos(yaw) cos(pitch))
		this.x = targetX + Math.sin(yr) * cos * this.distance;
		this.y = targetY + Math.sin(xr) * this.distance;
		this.z = targetZ - Math.cos(yr) * cos * this.distance;
		this.yRot = this.yaw;
		this.xRot = this.pitch;
	}
}
