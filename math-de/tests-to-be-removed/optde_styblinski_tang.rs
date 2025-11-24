use autoeq_de::{DEConfig, Mutation, run_recorded_differential_evolution};
use autoeq_testfunctions::styblinski_tang2;

#[test]
fn test_de_styblinski_tang() {
    let b = vec![(-5.0, 5.0), (-5.0, 5.0)];
    let mut c = DEConfig::default();
    c.seed = Some(13);
    c.maxiter = 800;
    c.popsize = 30;
    c.mutation = Mutation::Range { min: 0.5, max: 1.2 };
    {
        let result =
            run_recorded_differential_evolution("styblinski_tang", styblinski_tang2, &b, c);
        assert!(result.is_ok());
        let (report, _csv_path) = result.unwrap();
        assert!(report.fun < -70.0)
    };
}
