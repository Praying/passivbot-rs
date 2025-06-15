use crate::backtest;
use crate::types::{Analysis, BotConfig, BotSideConfig};
use rand::prelude::*;
use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tracing::info;
use crate::exchange::SendSyncError;

// --- Core NSGA-II Data Structures ---

#[derive(Debug, Clone)]
pub struct Individual {
    pub variables: Vec<f64>,
    pub fitness: Vec<f64>,
    pub rank: i32,
    pub crowding_distance: f64,
}

impl Individual {
    fn new(variables: Vec<f64>) -> Self {
        Self {
            variables,
            fitness: Vec::new(),
            rank: i32::MAX,
            crowding_distance: 0.0,
        }
    }

    fn dominates(&self, other: &Self) -> bool {
        let mut self_is_better = false;
        for i in 0..self.fitness.len() {
            if self.fitness[i] > other.fitness[i] {
                return false;
            }
            if self.fitness[i] < other.fitness[i] {
                self_is_better = true;
            }
        }
        self_is_better
    }
}

// --- NSGA-II Algorithm Logic (as free functions) ---

fn evaluate_population(
    population: &mut [Individual], base_config: &BotConfig, param_keys: &[String],
    tokio_runtime: &Arc<Runtime>, n_objectives: usize,
) {
    let rt = tokio_runtime.clone();
    population.par_iter_mut().for_each(|ind| {
        let config = individual_to_config(ind, base_config, param_keys);
        let backtest_result = rt.block_on(backtest::run_single(&config));
        match backtest_result {
            Ok(result) => {
                ind.fitness = calculate_fitness(&result.analysis);
            }
            Err(e) => {
                eprintln!("Backtest failed for individual. Error: {}", e);
                ind.fitness = vec![f64::MAX; n_objectives];
            }
        }
    });
}

fn fast_non_dominated_sort(population: &mut [Individual]) -> Vec<Vec<Individual>> {
    let n = population.len();
    let mut dominance_counts = vec![0; n];
    let mut dominated_solutions: Vec<Vec<usize>> = vec![Vec::new(); n];

    for i in 0..n {
        for j in (i + 1)..n {
            if population[i].dominates(&population[j]) {
                dominated_solutions[i].push(j);
                dominance_counts[j] += 1;
            } else if population[j].dominates(&population[i]) {
                dominated_solutions[j].push(i);
                dominance_counts[i] += 1;
            }
        }
    }

    let mut fronts: Vec<Vec<Individual>> = Vec::new();
    let mut current_front_indices: Vec<usize> =
        (0..n).filter(|&i| dominance_counts[i] == 0).collect();

    let mut rank = 1;
    while !current_front_indices.is_empty() {
        for &idx in &current_front_indices {
            population[idx].rank = rank;
        }
        fronts.push(
            current_front_indices
                .iter()
                .map(|&i| population[i].clone())
                .collect(),
        );

        let mut next_front_indices = Vec::new();
        for &p_idx in &current_front_indices {
            for &q_idx in &dominated_solutions[p_idx] {
                dominance_counts[q_idx] -= 1;
                if dominance_counts[q_idx] == 0 {
                    next_front_indices.push(q_idx);
                }
            }
        }
        current_front_indices = next_front_indices;
        rank += 1;
    }
    fronts
}

fn crowding_distance_assignment(front: &mut [Individual], n_objectives: usize) {
    if front.is_empty() {
        return;
    }
    let len = front.len();
    for ind in front.iter_mut() {
        ind.crowding_distance = 0.0;
    }

    for i in 0..n_objectives {
        front.sort_by(|a, b| {
            a.fitness[i]
                .partial_cmp(&b.fitness[i])
                .unwrap_or(Ordering::Equal)
        });

        if len > 0 {
            front[0].crowding_distance = f64::INFINITY;
            front[len - 1].crowding_distance = f64::INFINITY;
        }

        if len > 2 {
            let min_obj = front[0].fitness[i];
            let max_obj = front[len - 1].fitness[i];
            let range = max_obj - min_obj;

            if range.abs() > 1e-9 {
                for j in 1..(len - 1) {
                    front[j].crowding_distance +=
                        (front[j + 1].fitness[i] - front[j - 1].fitness[i]) / range;
                }
            }
        }
    }
    front.sort_by(|a, b| {
        b.crowding_distance
            .partial_cmp(&a.crowding_distance)
            .unwrap_or(Ordering::Equal)
    });
}

fn tournament_selection<'a>(population: &'a [Individual], rng: &mut impl Rng) -> &'a Individual {
    let i1 = rng.gen_range(0..population.len());
    let i2 = rng.gen_range(0..population.len());
    let ind1 = &population[i1];
    let ind2 = &population[i2];

    if ind1.rank < ind2.rank {
        return ind1;
    }
    if ind2.rank < ind1.rank {
        return ind2;
    }
    if ind1.crowding_distance > ind2.crowding_distance {
        return ind1;
    }
    if ind2.crowding_distance > ind1.crowding_distance {
        return ind2;
    }

    if rng.gen() {
        ind1
    } else {
        ind2
    }
}

fn simulated_binary_crossover(
    parent1: &Individual, parent2: &Individual, crossover_prob: f64, eta_crossover: f64,
    bounds: &[(f64, f64)], rng: &mut impl Rng,
) -> (Individual, Individual) {
    let mut child1_vars = parent1.variables.clone();
    let mut child2_vars = parent2.variables.clone();

    if rng.gen::<f64>() > crossover_prob {
        return (Individual::new(child1_vars), Individual::new(child2_vars));
    }

    for i in 0..parent1.variables.len() {
        let u: f64 = rng.gen();
        let beta = if u <= 0.5 {
            (2.0 * u).powf(1.0 / (eta_crossover + 1.0))
        } else {
            (1.0 / (2.0 * (1.0 - u))).powf(1.0 / (eta_crossover + 1.0))
        };

        let v1 = 0.5 * ((1.0 + beta) * parent1.variables[i] + (1.0 - beta) * parent2.variables[i]);
        let v2 = 0.5 * ((1.0 - beta) * parent1.variables[i] + (1.0 + beta) * parent2.variables[i]);

        child1_vars[i] = v1.clamp(bounds[i].0, bounds[i].1);
        child2_vars[i] = v2.clamp(bounds[i].0, bounds[i].1);
    }
    (Individual::new(child1_vars), Individual::new(child2_vars))
}

fn polynomial_mutation(
    individual: &mut Individual, mutation_prob: f64, eta_mutation: f64, bounds: &[(f64, f64)],
    rng: &mut impl Rng,
) {
    for i in 0..individual.variables.len() {
        if rng.gen::<f64>() < mutation_prob {
            let u: f64 = rng.gen();
            let delta = if u < 0.5 {
                (2.0 * u).powf(1.0 / (eta_mutation + 1.0)) - 1.0
            } else {
                1.0 - (2.0 * (1.0 - u)).powf(1.0 / (eta_mutation + 1.0))
            };

            let val = individual.variables[i];
            let (low, high) = bounds[i];
            individual.variables[i] = (val + delta * (high - low)).clamp(low, high);
        }
    }
}

fn calculate_fitness(analysis: &Analysis) -> Vec<f64> {
    vec![-analysis.sharpe_ratio, analysis.drawdown_worst]
}

fn individual_to_config(
    individual: &Individual, base_config: &BotConfig, param_keys: &[String],
) -> BotConfig {
    let mut config = base_config.clone();
    let mut long_params = HashMap::new();
    let mut short_params = HashMap::new();

    for (i, key) in param_keys.iter().enumerate() {
        let value = individual.variables[i];
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() == 2 {
            let side = parts[0];
            let param_name = parts[1];
            if side == "long" {
                long_params.insert(param_name.to_string(), value);
            } else {
                short_params.insert(param_name.to_string(), value);
            }
        }
    }

    fn apply_params(side_config: &mut BotSideConfig, params: &HashMap<String, f64>) {
        macro_rules! set_param {
            ($field:ident) => {
                if let Some(v) = params.get(stringify!($field)) {
                    side_config.$field = *v;
                }
            };
        }
        set_param!(total_wallet_exposure_limit);
        set_param!(n_positions);
        set_param!(unstuck_loss_allowance_pct);
        set_param!(unstuck_close_pct);
        set_param!(unstuck_ema_dist);
        set_param!(unstuck_threshold);
        set_param!(filter_rolling_window);
        set_param!(filter_relative_volume_clip_pct);
        set_param!(ema_span_0);
        set_param!(ema_span_1);
        set_param!(entry_initial_qty_pct);
        set_param!(entry_initial_ema_dist);
        set_param!(entry_grid_spacing_pct);
        set_param!(entry_grid_spacing_weight);
        set_param!(entry_grid_double_down_factor);
        set_param!(entry_trailing_threshold_pct);
        set_param!(entry_trailing_retracement_pct);
        set_param!(entry_trailing_grid_ratio);
        set_param!(close_grid_min_markup);
        set_param!(close_grid_markup_range);
        set_param!(close_grid_qty_pct);
        set_param!(close_trailing_threshold_pct);
        set_param!(close_trailing_retracement_pct);
        set_param!(close_trailing_qty_pct);
        set_param!(close_trailing_grid_ratio);
    }

    apply_params(&mut config.bot.long, &long_params);
    apply_params(&mut config.bot.short, &short_params);
    config
}

// --- Main Optimizer Struct to be called from outside ---
pub struct Optimizer {
    pub config: BotConfig,
}

impl Optimizer {
    pub fn new(config: BotConfig) -> Self {
        Optimizer { config }
    }

    pub async fn start(&mut self) -> Result<(), SendSyncError> {
        info!("Starting custom NSGA-II optimizer...");

        let mut param_keys = Vec::new();
        let mut param_bounds = Vec::new();

        let optimizer_config = &self.config.optimizer;
        for (key, range) in optimizer_config.long.iter() {
            param_keys.push(format!("long.{}", key));
            param_bounds.push((range.start, range.end));
        }
        for (key, range) in optimizer_config.short.iter() {
            param_keys.push(format!("short.{}", key));
            param_bounds.push((range.start, range.end));
        }

        let population_size = optimizer_config.population_size as usize;
        let n_generations = optimizer_config.n_generations as usize;
        let n_variables = param_bounds.len();
        let mutation_prob = 1.0 / n_variables as f64;
        let crossover_prob = 0.9;
        let eta_mutation = 20.0;
        let eta_crossover = 20.0;
        let n_objectives = 2;

        let tokio_runtime = Arc::new(Runtime::new().map_err(|e| Box::new(e) as SendSyncError)?);
        let mut rng = thread_rng();

        // 1. Initialize Population
        let mut population: Vec<Individual> = (0..population_size)
            .map(|_| {
                let variables = param_bounds
                    .iter()
                    .map(|(low, high)| rng.gen_range(*low..=*high))
                    .collect();
                Individual::new(variables)
            })
            .collect();

        // 2. Evaluate initial population
        evaluate_population(
            &mut population,
            &self.config,
            &param_keys,
            &tokio_runtime,
            n_objectives,
        );

        // 3. Main generational loop
        for generation_idx in 0..n_generations {
            info!("Running generation {}...", generation_idx + 1);

            // 4. Create offspring
            let mut offspring = Vec::with_capacity(population_size);
            while offspring.len() < population_size {
                let parent1 = tournament_selection(&population, &mut rng);
                let parent2 = tournament_selection(&population, &mut rng);

                let (mut child1, mut child2) = simulated_binary_crossover(
                    parent1,
                    parent2,
                    crossover_prob,
                    eta_crossover,
                    &param_bounds,
                    &mut rng,
                );
                polynomial_mutation(
                    &mut child1,
                    mutation_prob,
                    eta_mutation,
                    &param_bounds,
                    &mut rng,
                );
                polynomial_mutation(
                    &mut child2,
                    mutation_prob,
                    eta_mutation,
                    &param_bounds,
                    &mut rng,
                );

                offspring.push(child1);
                if offspring.len() < population_size {
                    offspring.push(child2);
                }
            }

            // 5. Evaluate offspring
            evaluate_population(
                &mut offspring,
                &self.config,
                &param_keys,
                &tokio_runtime,
                n_objectives,
            );

            // 6. Combine and select next generation
            let mut combined_pop = population;
            combined_pop.append(&mut offspring);

            let mut fronts = fast_non_dominated_sort(&mut combined_pop);

            let mut next_pop = Vec::new();
            for front in fronts.iter_mut() {
                if next_pop.len() + front.len() > population_size {
                    crowding_distance_assignment(front, n_objectives);
                    let remaining = population_size - next_pop.len();
                    next_pop.extend_from_slice(&front[0..remaining]);
                    break;
                }
                next_pop.extend_from_slice(front);
            }
            population = next_pop;

            if let Some(best_ind) = population.get(0) {
                let best_fitness = best_ind.fitness.clone();
                info!(
                    "Generation {} Best Fitness (Negated Sharpe, Drawdown): {:?}",
                    generation_idx + 1,
                    best_fitness
                );
            }
        }

        // 7. Get final Pareto front
        let pareto_front = fast_non_dominated_sort(&mut population)
            .into_iter()
            .next()
            .unwrap_or_default();

        info!("Optimization finished!");
        info!(
            "Found {} solutions in the Pareto front.",
            pareto_front.len()
        );

        for (i, individual) in pareto_front.iter().enumerate() {
            let sharpe = -individual.fitness[0]; // Negate back
            let drawdown = individual.fitness[1];
            info!(
                "Solution {}: Sharpe Ratio = {:.4}, Worst Drawdown = {:.4}%",
                i + 1,
                sharpe,
                drawdown * 100.0
            );
            if i < 5 {
                // Print top 5 configs
                let config = individual_to_config(individual, &self.config, &param_keys);
                info!("Config for solution {}: {:#?}", i + 1, config.bot);
            }
        }
        Ok(())
    }
}
