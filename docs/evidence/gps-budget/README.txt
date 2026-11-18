The GPS route search's cost and the frame budget (docs/formats.md, "GPS",
Frame budget).

survey-summary.tsv: per career stage's network, the searches of the data
test `ai::gps::data_tests::search_cost_survey` (run with `--ignored`,
GPS_SURVEY_STAGE=1..5, GPS_SURVEY_OUT=<scratch dir>/survey-stageN.tsv; the
raw per-step tables are not in the repository) summarised: nodes, unloaded
wall time (least of five runs per step), the frames the original's 0.6 ms
slices take on those times, and the frames the clone's 700000 work units a
frame take, with the share of searches where both agree.
