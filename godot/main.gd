extends Node3D

@onready var world_gen: SyklaWorldGenerator = $WorldGenerator
@onready var camera: Camera3D = $Camera3D
@onready var sun: DirectionalLight3D = $DirectionalLight3D

var route: SyklaRoute
var cam_dist := 0.0

const SAMPLE_GPX := '<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="sykla" xmlns="http://www.topografix.com/GPX/1/1">
  <trk>
    <name>Downtown City Ride</name>
    <trkseg>
      <trkpt lat="35.7796" lon="-78.6382"><ele>100.0</ele></trkpt>
      <trkpt lat="35.7798" lon="-78.6380"><ele>100.2</ele></trkpt>
      <trkpt lat="35.7800" lon="-78.6378"><ele>100.5</ele></trkpt>
      <trkpt lat="35.7802" lon="-78.6376"><ele>100.8</ele></trkpt>
      <trkpt lat="35.7804" lon="-78.6374"><ele>101.2</ele></trkpt>
      <trkpt lat="35.7806" lon="-78.6372"><ele>101.8</ele></trkpt>
      <trkpt lat="35.7808" lon="-78.6370"><ele>102.5</ele></trkpt>
      <trkpt lat="35.7810" lon="-78.6368"><ele>103.0</ele></trkpt>
      <trkpt lat="35.7812" lon="-78.6366"><ele>103.2</ele></trkpt>
      <trkpt lat="35.7814" lon="-78.6364"><ele>103.5</ele></trkpt>
      <trkpt lat="35.7816" lon="-78.6362"><ele>103.8</ele></trkpt>
      <trkpt lat="35.7818" lon="-78.6360"><ele>104.0</ele></trkpt>
      <trkpt lat="35.7820" lon="-78.6358"><ele>104.2</ele></trkpt>
      <trkpt lat="35.7822" lon="-78.6356"><ele>104.0</ele></trkpt>
      <trkpt lat="35.7824" lon="-78.6354"><ele>103.8</ele></trkpt>
      <trkpt lat="35.7826" lon="-78.6352"><ele>103.5</ele></trkpt>
      <trkpt lat="35.7828" lon="-78.6350"><ele>103.2</ele></trkpt>
      <trkpt lat="35.7830" lon="-78.6348"><ele>103.0</ele></trkpt>
      <trkpt lat="35.7832" lon="-78.6346"><ele>102.8</ele></trkpt>
      <trkpt lat="35.7834" lon="-78.6344"><ele>102.5</ele></trkpt>
      <trkpt lat="35.7836" lon="-78.6342"><ele>102.2</ele></trkpt>
      <trkpt lat="35.7838" lon="-78.6340"><ele>102.0</ele></trkpt>
      <trkpt lat="35.7840" lon="-78.6338"><ele>101.8</ele></trkpt>
      <trkpt lat="35.7842" lon="-78.6336"><ele>101.5</ele></trkpt>
    </trkseg>
  </trk>
</gpx>'

func _ready() -> void:
	route = SyklaRoute.new()
	if not route.load_gpx(SAMPLE_GPX):
		push_error("Failed to load GPX")
		return

	print("Route: ", route.get_name())
	print("  Points: ", route.get_point_count())
	print("  Distance: %.0f m" % route.get_total_distance_m())

	world_gen.generate_world(route)

	# Warm afternoon sun — low angle for long shadows between buildings
	sun.rotation_degrees = Vector3(-25, -50, 0)
	sun.light_energy = 1.3
	sun.light_color = Color(1.0, 0.93, 0.82)
	sun.shadow_enabled = true
	sun.light_size = 0.3

	_position_camera(0.0)

func _process(delta: float) -> void:
	cam_dist += delta * 6.0
	if cam_dist > route.get_total_distance_m() - 30.0:
		cam_dist = 0.0
	_position_camera(cam_dist)

func _position_camera(dist: float) -> void:
	var elev := route.elevation_at_distance(dist)
	var look_ahead := minf(dist + 25.0, route.get_total_distance_m())
	var look_elev := route.elevation_at_distance(look_ahead)

	# Street-level cyclist POV — slightly right of center, eye height ~1.6m
	camera.position = Vector3(1.5, elev + 1.6, dist)
	camera.look_at(Vector3(0.5, look_elev + 1.4, look_ahead), Vector3.UP)
	camera.fov = 70.0
	camera.far = 500.0
	camera.near = 0.15
